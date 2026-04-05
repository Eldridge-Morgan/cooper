use crate::{DeployPlan, DeployResult, ProvisionedResource};
use anyhow::{Context, Result};
use cooper_codegen::analyzer::ProjectAnalysis;
use std::process::Stdio;
use tokio::process::Command;

/// GCP provisioner — creates Cloud Run, Cloud SQL, Pub/Sub, Memorystore, GCS resources.
///
/// Uses the `gcloud` CLI for authentication and API calls.
pub struct GcpProvisioner {
    project_id: String,
    region: String,
}

impl GcpProvisioner {
    pub fn new() -> Result<Self> {
        let project_id = std::env::var("GOOGLE_CLOUD_PROJECT")
            .or_else(|_| std::env::var("GCP_PROJECT"))
            .or_else(|_| std::env::var("GCLOUD_PROJECT"))
            .unwrap_or_default();

        let region = std::env::var("GOOGLE_CLOUD_REGION")
            .unwrap_or_else(|_| "us-central1".to_string());

        Ok(Self { project_id, region })
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

        // Resolve project ID if not set
        let project_id = if self.project_id.is_empty() {
            let output = self.gcloud(&["config", "get-value", "project"]).await?;
            output.trim().to_string()
        } else {
            self.project_id.clone()
        };

        // 1. Enable required APIs
        for api in &[
            "run.googleapis.com",
            "sqladmin.googleapis.com",
            "pubsub.googleapis.com",
            "redis.googleapis.com",
            "storage.googleapis.com",
        ] {
            let _ = self.gcloud(&["services", "enable", api, "--project", &project_id]).await;
        }

        // 2. Cloud SQL instances
        for db in &analysis.databases {
            let instance_name = format!("{prefix}-{}", db.name);
            let db_version = match db.engine.as_str() {
                "mysql" => "MYSQL_8_0",
                _ => "POSTGRES_15",
            };

            tracing::info!("Creating Cloud SQL instance: {instance_name}");
            let _ = self.gcloud(&[
                "sql", "instances", "create", &instance_name,
                "--database-version", db_version,
                "--tier", "db-f1-micro",
                "--region", &self.region,
                "--project", &project_id,
                "--root-password", &format!("cooper-{env}"),
                "--async",
            ]).await;

            // Create database
            let _ = self.gcloud(&[
                "sql", "databases", "create", &format!("cooper_{}", db.name),
                "--instance", &instance_name,
                "--project", &project_id,
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "Cloud SQL".to_string(),
                name: instance_name.clone(),
                status: "creating".to_string(),
                connection_info: Some(format!("{project_id}:{0}:{instance_name}", self.region)),
            });
        }

        // 3. Memorystore Redis
        {
            let redis_name = format!("{prefix}-cache");
            tracing::info!("Creating Memorystore: {redis_name}");
            let _ = self.gcloud(&[
                "redis", "instances", "create", &redis_name,
                "--size", "1",
                "--region", &self.region,
                "--project", &project_id,
                "--async",
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "Memorystore".to_string(),
                name: redis_name,
                status: "creating".to_string(),
                connection_info: None,
            });
        }

        // 4. Pub/Sub topics
        for topic in &analysis.topics {
            let topic_name = format!("{prefix}-{}", topic.name);
            tracing::info!("Creating Pub/Sub topic: {topic_name}");
            let _ = self.gcloud(&[
                "pubsub", "topics", "create", &topic_name,
                "--project", &project_id,
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "Pub/Sub Topic".to_string(),
                name: topic_name,
                status: "active".to_string(),
                connection_info: None,
            });
        }

        // 5. GCS bucket
        {
            let bucket_name = format!("{prefix}-storage");
            tracing::info!("Creating GCS bucket: {bucket_name}");
            let _ = self.gcloud(&[
                "storage", "buckets", "create", &format!("gs://{bucket_name}"),
                "--location", &self.region,
                "--project", &project_id,
            ]).await;

            resources.push(ProvisionedResource {
                resource_type: "GCS Bucket".to_string(),
                name: bucket_name,
                status: "active".to_string(),
                connection_info: None,
            });
        }

        // 6. Cloud Run service
        {
            let service_name = format!("{prefix}-app");
            tracing::info!("Deploying Cloud Run service: {service_name}");

            // Build and push container image
            let image = format!("gcr.io/{project_id}/{service_name}:latest");

            let _ = self.gcloud(&[
                "run", "deploy", &service_name,
                "--image", &image,
                "--platform", "managed",
                "--region", &self.region,
                "--allow-unauthenticated",
                "--port", "4000",
                "--memory", "512Mi",
                "--project", &project_id,
            ]).await;

            // Get the service URL
            let url_output = self.gcloud(&[
                "run", "services", "describe", &service_name,
                "--platform", "managed",
                "--region", &self.region,
                "--project", &project_id,
                "--format", "value(status.url)",
            ]).await.unwrap_or_default();

            let service_url = url_output.trim().to_string();

            resources.push(ProvisionedResource {
                resource_type: "Cloud Run".to_string(),
                name: service_name,
                status: "serving".to_string(),
                connection_info: Some(service_url.clone()),
            });

            // Save state
            self.save_state(env, project_name, &resources)?;

            return Ok(DeployResult {
                env: env.to_string(),
                provider: "gcp".to_string(),
                url: Some(service_url),
                resources,
            });
        }
    }

    pub async fn destroy(&self, env: &str, project_name: &str) -> Result<()> {
        let prefix = format!("cooper-{project_name}-{env}");
        let project_id = if self.project_id.is_empty() {
            let output = self.gcloud(&["config", "get-value", "project"]).await?;
            output.trim().to_string()
        } else {
            self.project_id.clone()
        };

        tracing::info!("Destroying GCP environment: {prefix}");

        let _ = self.gcloud(&["run", "services", "delete", &format!("{prefix}-app"),
            "--platform", "managed", "--region", &self.region,
            "--project", &project_id, "--quiet"]).await;

        let _ = self.gcloud(&["sql", "instances", "delete", &format!("{prefix}-main"),
            "--project", &project_id, "--quiet"]).await;

        let _ = self.gcloud(&["redis", "instances", "delete", &format!("{prefix}-cache"),
            "--region", &self.region, "--project", &project_id, "--quiet"]).await;

        Ok(())
    }

    fn save_state(&self, env: &str, project_name: &str, resources: &[ProvisionedResource]) -> Result<()> {
        let state = serde_json::json!({
            "env": env,
            "project": project_name,
            "provider": "gcp",
            "project_id": self.project_id,
            "region": self.region,
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

    async fn gcloud(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("gcloud")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("gcloud CLI not found. Install: https://cloud.google.com/sdk")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("gcloud error: {}", stderr.trim()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}
