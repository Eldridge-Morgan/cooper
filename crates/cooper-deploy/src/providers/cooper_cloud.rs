use crate::{DeployPlan, DeployResult, ProvisionedResource};
use anyhow::{Context, Result};
use cooper_codegen::analyzer::ProjectAnalysis;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const API_BASE: &str = "https://api.coopercloud.io";

/// Cooper Cloud provider — deploys to Cooper's managed infrastructure.
///
/// Flow:
/// 1. Authenticate with API key
/// 2. Build image locally
/// 3. Push image to Cooper Cloud ECR
/// 4. POST /v1/deploy with manifest
/// 5. Control plane creates namespace, deployment, ingress
/// 6. Returns URL
pub struct CooperCloudProvisioner {
    client: Client,
    api_base: String,
}

#[derive(Serialize)]
struct DeployRequest {
    env: String,
    image: String,
    manifest: serde_json::Value,
}

#[derive(Deserialize)]
struct DeployResponse {
    url: String,
    status: String,
}

#[derive(Deserialize)]
struct AuthResponse {
    token: String,
    tenant_id: String,
    registry: String,
}

impl CooperCloudProvisioner {
    pub fn new() -> Self {
        let api_base = std::env::var("COOPER_CLOUD_API")
            .unwrap_or_else(|_| API_BASE.to_string());

        Self {
            client: Client::new(),
            api_base,
        }
    }

    pub async fn provision(
        &self,
        _plan: &DeployPlan,
        analysis: &ProjectAnalysis,
        env: &str,
        project_name: &str,
    ) -> Result<DeployResult> {
        // 1. Authenticate
        let api_key = self.get_api_key()?;
        let auth = self.authenticate(&api_key).await?;

        tracing::info!("Authenticated as tenant {}", auth.tenant_id);

        // 2. Build image
        tracing::info!("Building production image...");
        let image_tag = format!(
            "{}/{}/{}:{}",
            auth.registry, auth.tenant_id, project_name,
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        );
        self.build_image(&image_tag).await?;

        // 3. Push image
        tracing::info!("Pushing image to Cooper Cloud registry...");
        self.push_image(&image_tag).await?;

        // 4. Deploy
        tracing::info!("Deploying to Cooper Cloud...");
        let manifest = serde_json::json!({
            "routes": analysis.routes,
            "databases": analysis.databases,
            "topics": analysis.topics,
            "queues": analysis.queues,
            "crons": analysis.crons,
        });

        let deploy_resp = self
            .client
            .post(format!("{}/v1/deploy", self.api_base))
            .bearer_auth(&auth.token)
            .json(&DeployRequest {
                env: env.to_string(),
                image: image_tag.clone(),
                manifest,
            })
            .send()
            .await
            .context("Failed to reach Cooper Cloud API")?
            .json::<DeployResponse>()
            .await
            .context("Invalid response from Cooper Cloud")?;

        let mut resources = vec![
            ProvisionedResource {
                resource_type: "Cooper Cloud Service".to_string(),
                name: project_name.to_string(),
                status: deploy_resp.status.clone(),
                connection_info: Some(deploy_resp.url.clone()),
            },
        ];

        // Add database, cache, etc. as resources
        if !analysis.databases.is_empty() {
            resources.push(ProvisionedResource {
                resource_type: "Managed Postgres".to_string(),
                name: format!("{project_name}-db"),
                status: "available".to_string(),
                connection_info: Some("schema-isolated on shared RDS".to_string()),
            });
        }

        if !analysis.topics.is_empty() || !analysis.queues.is_empty() {
            resources.push(ProvisionedResource {
                resource_type: "Managed NATS".to_string(),
                name: format!("{project_name}-messaging"),
                status: "available".to_string(),
                connection_info: Some("namespace-isolated on shared NATS".to_string()),
            });
        }

        // Save state
        self.save_state(env, project_name, &resources, &deploy_resp.url)?;

        Ok(DeployResult {
            env: env.to_string(),
            provider: "cooper".to_string(),
            url: Some(deploy_resp.url),
            resources,
        })
    }

    pub async fn destroy(&self, env: &str, _project_name: &str) -> Result<()> {
        let api_key = self.get_api_key()?;
        let auth = self.authenticate(&api_key).await?;

        self.client
            .delete(format!("{}/v1/environments/{}", self.api_base, env))
            .bearer_auth(&auth.token)
            .send()
            .await
            .context("Failed to destroy environment")?;

        // Clean local state
        let _ = std::fs::remove_dir_all(format!(".cooper/state/{env}"));

        Ok(())
    }

    async fn authenticate(&self, api_key: &str) -> Result<AuthResponse> {
        self.client
            .post(format!("{}/v1/auth/login", self.api_base))
            .json(&serde_json::json!({ "api_key": api_key }))
            .send()
            .await
            .context("Failed to authenticate with Cooper Cloud")?
            .json::<AuthResponse>()
            .await
            .context("Invalid auth response")
    }

    fn get_api_key(&self) -> Result<String> {
        // Check env var first, then config file
        if let Ok(key) = std::env::var("COOPER_CLOUD_API_KEY") {
            return Ok(key);
        }

        let config_path = dirs_next::home_dir()
            .map(|h| h.join(".cooper/cloud-credentials"))
            .unwrap_or_default();

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            if let Some(key) = content.lines().next() {
                return Ok(key.trim().to_string());
            }
        }

        Err(anyhow::anyhow!(
            "Not logged in to Cooper Cloud. Run `cooper login` or set COOPER_CLOUD_API_KEY."
        ))
    }

    async fn build_image(&self, _tag: &str) -> Result<()> {
        // Build using cooper build, then docker build
        let status = tokio::process::Command::new("cooper")
            .args(["build"])
            .status()
            .await?;

        if !status.success() {
            return Err(anyhow::anyhow!("Build failed"));
        }

        Ok(())
    }

    async fn push_image(&self, tag: &str) -> Result<()> {
        let status = tokio::process::Command::new("docker")
            .args(["push", tag])
            .status()
            .await
            .context("docker push failed — is Docker running?")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Image push failed"));
        }

        Ok(())
    }

    fn save_state(
        &self,
        env: &str,
        project_name: &str,
        resources: &[ProvisionedResource],
        url: &str,
    ) -> Result<()> {
        let state = serde_json::json!({
            "env": env,
            "project": project_name,
            "provider": "cooper",
            "url": url,
            "resources": resources,
            "deployed_at": chrono::Utc::now().to_rfc3339(),
        });

        let state_dir = format!(".cooper/state/{env}");
        std::fs::create_dir_all(&state_dir)?;
        std::fs::write(
            format!("{state_dir}/deploy.json"),
            serde_json::to_string_pretty(&state)?,
        )?;

        Ok(())
    }
}
