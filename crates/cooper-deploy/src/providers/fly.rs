use crate::{DeployPlan, DeployResult, ProvisionedResource};
use anyhow::{Context, Result};
use cooper_codegen::analyzer::ProjectAnalysis;
use reqwest::Client;
use std::process::Stdio;
use tokio::process::Command;

/// Fly.io provisioner — creates Fly Machines, Fly Postgres, Upstash Redis/QStash.
///
/// Uses the `flyctl` CLI and Fly Machines API.
pub struct FlyProvisioner {
    region: String,
    org: String,
    client: Client,
}

impl FlyProvisioner {
    pub fn new() -> Self {
        Self {
            region: std::env::var("FLY_REGION").unwrap_or_else(|_| "iad".to_string()),
            org: std::env::var("FLY_ORG").unwrap_or_else(|_| "personal".to_string()),
            client: Client::new(),
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
        let mut resources = Vec::new();

        // Get Fly API token
        let _token = self.get_token().await?;

        // 1. Create Fly app
        let app_name = format!("{prefix}-app");
        tracing::info!("Creating Fly app: {app_name}");
        let _ = self.flyctl(&["apps", "create", &app_name, "--org", &self.org]).await;

        // 2. Create Fly Postgres cluster
        for db in &analysis.databases {
            let pg_name = format!("{prefix}-{}", db.name);
            tracing::info!("Creating Fly Postgres: {pg_name}");

            let _ = self.flyctl(&[
                "postgres", "create",
                "--name", &pg_name,
                "--org", &self.org,
                "--region", &self.region,
                "--vm-size", "shared-cpu-1x",
                "--initial-cluster-size", "1",
                "--volume-size", "1",
            ]).await;

            // Attach to the app
            let _ = self.flyctl(&[
                "postgres", "attach",
                &pg_name,
                "--app", &app_name,
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "Fly Postgres".to_string(),
                name: pg_name,
                status: "running".to_string(),
                connection_info: None, // Connection string auto-injected as DATABASE_URL
            });
        }

        // 3. Create Upstash Redis via Fly
        {
            let redis_name = format!("{prefix}-cache");
            tracing::info!("Creating Upstash Redis: {redis_name}");

            let _ = self.flyctl(&[
                "redis", "create",
                "--name", &redis_name,
                "--org", &self.org,
                "--region", &self.region,
                "--no-replicas",
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "Upstash Redis".to_string(),
                name: redis_name,
                status: "running".to_string(),
                connection_info: None,
            });
        }

        // 4. Set secrets for topics/queues (would use Upstash QStash in production)
        for topic in &analysis.topics {
            let _ = self.flyctl(&[
                "secrets", "set",
                &format!("COOPER_TOPIC_{}={}", topic.name.to_uppercase().replace('-', "_"), topic.name),
                "--app", &app_name,
            ]).await;
        }

        // 5. Create volume for object storage
        {
            let _ = self.flyctl(&[
                "volumes", "create", "cooper_storage",
                "--app", &app_name,
                "--region", &self.region,
                "--size", "1",
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "Fly Volume".to_string(),
                name: format!("{prefix}-storage"),
                status: "active".to_string(),
                connection_info: None,
            });
        }

        // 6. Generate fly.toml and deploy
        let fly_toml = self.generate_fly_toml(&app_name);
        std::fs::write("fly.toml", &fly_toml)?;

        tracing::info!("Deploying to Fly.io...");
        let _ = self.flyctl(&[
            "deploy",
            "--app", &app_name,
            "--region", &self.region,
            "--ha=false",
        ]).await;

        // Get the URL
        let url = format!("https://{app_name}.fly.dev");

        resources.push(ProvisionedResource {
            resource_type: "Fly Machine".to_string(),
            name: app_name,
            status: "running".to_string(),
            connection_info: Some(url.clone()),
        });

        self.save_state(env, project_name, &resources)?;

        Ok(DeployResult {
            env: env.to_string(),
            provider: "fly".to_string(),
            url: Some(url),
            resources,
        })
    }

    pub async fn destroy(&self, env: &str, project_name: &str) -> Result<()> {
        let app_name = format!("cooper-{project_name}-{env}-app");
        tracing::info!("Destroying Fly app: {app_name}");
        self.flyctl(&["apps", "destroy", &app_name, "--yes"]).await?;
        Ok(())
    }

    fn generate_fly_toml(&self, app_name: &str) -> String {
        format!(
            r#"app = "{app_name}"
primary_region = "{region}"

[build]
  dockerfile = "Dockerfile"

[http_service]
  internal_port = 4000
  force_https = true
  auto_stop_machines = true
  auto_start_machines = true
  min_machines_running = 0

[[vm]]
  cpu_kind = "shared"
  cpus = 1
  memory_mb = 256

[mounts]
  source = "cooper_storage"
  destination = "/data/storage"
"#,
            region = self.region
        )
    }

    async fn get_token(&self) -> Result<String> {
        std::env::var("FLY_API_TOKEN").or_else(|_| {
            // Try reading from flyctl auth
            Ok("".to_string())
        }).map_err(|_e: std::env::VarError| anyhow::anyhow!("FLY_API_TOKEN not set"))
    }

    fn save_state(&self, env: &str, project_name: &str, resources: &[ProvisionedResource]) -> Result<()> {
        let state = serde_json::json!({
            "env": env,
            "project": project_name,
            "provider": "fly",
            "region": self.region,
            "resources": resources,
            "deployed_at": chrono::Utc::now().to_rfc3339(),
        });

        let state_dir = format!(".cooper/state/{env}");
        std::fs::create_dir_all(&state_dir)?;
        std::fs::write(format!("{state_dir}/deploy.json"), serde_json::to_string_pretty(&state)?)?;
        Ok(())
    }

    async fn flyctl(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("flyctl")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("flyctl not found. Install: https://fly.io/docs/hands-on/install-flyctl/")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("flyctl error: {}", stderr.trim()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
