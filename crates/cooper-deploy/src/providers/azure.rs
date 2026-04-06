use crate::{DeployPlan, DeployResult, ProvisionedResource};
use anyhow::{Context, Result};
use cooper_codegen::analyzer::ProjectAnalysis;
use std::process::Stdio;
use tokio::process::Command;

/// Azure provisioner — creates Container Apps, Azure DB, Service Bus, Redis, Blob Storage.
pub struct AzureProvisioner {
    location: String,
}

impl AzureProvisioner {
    pub fn new() -> Self {
        Self {
            location: std::env::var("AZURE_LOCATION")
                .unwrap_or_else(|_| "eastus".to_string()),
        }
    }

    pub async fn provision(
        &self,
        _plan: &DeployPlan,
        analysis: &ProjectAnalysis,
        env: &str,
        project_name: &str,
    ) -> Result<DeployResult> {
        let prefix = format!("cooper-{project_name}-{env}");
        let rg = format!("{prefix}-rg");
        let mut resources = Vec::new();

        // 1. Create resource group
        tracing::info!("Creating resource group: {rg}");
        self.az(&["group", "create", "--name", &rg, "--location", &self.location]).await?;

        // 2. Azure Database for PostgreSQL
        for db in &analysis.databases {
            let server_name = format!("{prefix}-{}", db.name);
            tracing::info!("Creating Azure DB: {server_name}");

            let _ = self.az(&[
                "postgres", "flexible-server", "create",
                "--resource-group", &rg,
                "--name", &server_name,
                "--location", &self.location,
                "--admin-user", "cooper",
                "--admin-password", &format!("Cooper{}!", chrono::Utc::now().timestamp() % 1_000_000),
                "--sku-name", "Standard_B1ms",
                "--tier", "Burstable",
                "--storage-size", "32",
                "--version", "15",
                "--yes",
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "Azure PostgreSQL".to_string(),
                name: server_name.clone(),
                status: "available".to_string(),
                connection_info: Some(format!("{server_name}.postgres.database.azure.com")),
            });
        }

        // 3. Azure Cache for Redis
        {
            let redis_name = format!("{prefix}-cache");
            tracing::info!("Creating Azure Redis: {redis_name}");

            let _ = self.az(&[
                "redis", "create",
                "--resource-group", &rg,
                "--name", &redis_name,
                "--location", &self.location,
                "--sku", "Basic",
                "--vm-size", "c0",
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "Azure Redis".to_string(),
                name: redis_name.clone(),
                status: "creating".to_string(),
                connection_info: Some(format!("{redis_name}.redis.cache.windows.net")),
            });
        }

        // 4. Service Bus for messaging
        {
            let sb_name = format!("{prefix}-bus");
            tracing::info!("Creating Service Bus: {sb_name}");

            let _ = self.az(&[
                "servicebus", "namespace", "create",
                "--resource-group", &rg,
                "--name", &sb_name,
                "--location", &self.location,
                "--sku", "Basic",
            ]).await;

            for topic in &analysis.topics {
                let _ = self.az(&[
                    "servicebus", "queue", "create",
                    "--resource-group", &rg,
                    "--namespace-name", &sb_name,
                    "--name", &topic.name,
                ]).await;
            }

            resources.push(ProvisionedResource {
                resource_type: "Service Bus".to_string(),
                name: sb_name,
                status: "active".to_string(),
                connection_info: None,
            });
        }

        // 5. Storage account + blob container
        {
            // Azure storage names: lowercase, no hyphens, 3-24 chars
            let storage_name: String = format!("cooper{project_name}{env}")
                .chars()
                .filter(|c| c.is_alphanumeric())
                .take(24)
                .collect();
            tracing::info!("Creating Storage Account: {storage_name}");

            let _ = self.az(&[
                "storage", "account", "create",
                "--resource-group", &rg,
                "--name", &storage_name,
                "--location", &self.location,
                "--sku", "Standard_LRS",
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "Blob Storage".to_string(),
                name: storage_name,
                status: "active".to_string(),
                connection_info: None,
            });
        }

        // 6. Container Apps environment + app
        {
            let env_name = format!("{prefix}-env");
            let app_name = format!("{prefix}-app");
            tracing::info!("Creating Container App: {app_name}");

            let _ = self.az(&[
                "containerapp", "env", "create",
                "--resource-group", &rg,
                "--name", &env_name,
                "--location", &self.location,
            ]).await;

            let _ = self.az(&[
                "containerapp", "create",
                "--resource-group", &rg,
                "--name", &app_name,
                "--environment", &env_name,
                "--target-port", "4000",
                "--ingress", "external",
                "--min-replicas", "0",
                "--max-replicas", "10",
            ]).await;

            // Get FQDN
            let url_output = self.az(&[
                "containerapp", "show",
                "--resource-group", &rg,
                "--name", &app_name,
                "--query", "properties.configuration.ingress.fqdn",
                "--output", "tsv",
            ]).await.unwrap_or_default();

            let url = format!("https://{}", url_output.trim());

            resources.push(ProvisionedResource {
                resource_type: "Container App".to_string(),
                name: app_name,
                status: "running".to_string(),
                connection_info: Some(url.clone()),
            });

            self.save_state(env, project_name, &resources)?;

            return Ok(DeployResult {
                env: env.to_string(),
                provider: "azure".to_string(),
                url: Some(url),
                resources,
            });
        }
    }

    pub async fn destroy(&self, env: &str, project_name: &str) -> Result<()> {
        let rg = format!("cooper-{project_name}-{env}-rg");
        tracing::info!("Destroying Azure resource group: {rg}");
        self.az(&["group", "delete", "--name", &rg, "--yes", "--no-wait"]).await?;
        Ok(())
    }

    fn save_state(&self, env: &str, project_name: &str, resources: &[ProvisionedResource]) -> Result<()> {
        let state = serde_json::json!({
            "env": env,
            "project": project_name,
            "provider": "azure",
            "location": self.location,
            "resources": resources,
            "deployed_at": chrono::Utc::now().to_rfc3339(),
        });

        let state_dir = format!(".cooper/state/{env}");
        std::fs::create_dir_all(&state_dir)?;
        std::fs::write(format!("{state_dir}/deploy.json"), serde_json::to_string_pretty(&state)?)?;
        Ok(())
    }

    async fn az(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("az")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("Azure CLI not found. Install: https://learn.microsoft.com/en-us/cli/azure/install-azure-cli")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("az error: {}", stderr.trim()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
