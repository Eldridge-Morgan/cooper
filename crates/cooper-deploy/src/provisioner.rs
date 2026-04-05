use crate::{CloudProvider, DeployPlan};
use anyhow::Result;

/// Execute a deploy plan against the target cloud
pub async fn provision(provider: &CloudProvider, plan: &DeployPlan, env: &str) -> Result<()> {
    match provider {
        CloudProvider::Aws => provision_aws(plan, env).await,
        CloudProvider::Gcp => provision_gcp(plan, env).await,
        CloudProvider::Azure => provision_azure(plan, env).await,
        CloudProvider::Fly => provision_fly(plan, env).await,
    }
}

async fn provision_aws(_plan: &DeployPlan, _env: &str) -> Result<()> {
    // TODO: AWS SDK calls — ECS, RDS, SQS, ElastiCache, S3
    tracing::info!("Provisioning AWS resources...");
    Ok(())
}

async fn provision_gcp(_plan: &DeployPlan, _env: &str) -> Result<()> {
    tracing::info!("Provisioning GCP resources...");
    Ok(())
}

async fn provision_azure(_plan: &DeployPlan, _env: &str) -> Result<()> {
    tracing::info!("Provisioning Azure resources...");
    Ok(())
}

async fn provision_fly(_plan: &DeployPlan, _env: &str) -> Result<()> {
    tracing::info!("Provisioning Fly.io resources...");
    Ok(())
}
