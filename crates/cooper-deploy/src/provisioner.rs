use crate::providers::{aws::AwsProvisioner, azure::AzureProvisioner, fly::FlyProvisioner, gcp::GcpProvisioner};
use crate::{CloudProvider, DeployPlan, DeployResult};
use anyhow::Result;
use cooper_codegen::analyzer::ProjectAnalysis;

/// Execute a deploy plan against the target cloud provider.
pub async fn provision(
    provider: &CloudProvider,
    plan: &DeployPlan,
    analysis: &ProjectAnalysis,
    env: &str,
    project_name: &str,
) -> Result<DeployResult> {
    match provider {
        CloudProvider::Aws => {
            let p = AwsProvisioner::new();
            p.provision(plan, analysis, env, project_name).await
        }
        CloudProvider::Gcp => {
            let p = GcpProvisioner::new()?;
            p.provision(plan, analysis, env, project_name).await
        }
        CloudProvider::Azure => {
            let p = AzureProvisioner::new();
            p.provision(plan, analysis, env, project_name).await
        }
        CloudProvider::Fly => {
            let p = FlyProvisioner::new();
            p.provision(plan, analysis, env, project_name).await
        }
    }
}

/// Destroy all resources in an environment.
pub async fn destroy(provider: &CloudProvider, env: &str, project_name: &str) -> Result<()> {
    match provider {
        CloudProvider::Aws => AwsProvisioner::new().destroy(env, project_name).await,
        CloudProvider::Gcp => GcpProvisioner::new()?.destroy(env, project_name).await,
        CloudProvider::Azure => AzureProvisioner::new().destroy(env, project_name).await,
        CloudProvider::Fly => FlyProvisioner::new().destroy(env, project_name).await,
    }
}
