use crate::providers::{
    aws::AwsProvisioner, azure::AzureProvisioner, cooper_cloud::CooperCloudProvisioner,
    fly::FlyProvisioner, gcp::GcpProvisioner,
};
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
        CloudProvider::Aws => AwsProvisioner::new().provision(plan, analysis, env, project_name).await,
        CloudProvider::Gcp => GcpProvisioner::new()?.provision(plan, analysis, env, project_name).await,
        CloudProvider::Azure => AzureProvisioner::new().provision(plan, analysis, env, project_name).await,
        CloudProvider::Fly => FlyProvisioner::new().provision(plan, analysis, env, project_name).await,
        CloudProvider::Cooper => CooperCloudProvisioner::new().provision(plan, analysis, env, project_name).await,
    }
}

/// Destroy all resources in an environment.
pub async fn destroy(provider: &CloudProvider, env: &str, project_name: &str) -> Result<()> {
    match provider {
        CloudProvider::Aws => AwsProvisioner::new().destroy(env, project_name).await,
        CloudProvider::Gcp => GcpProvisioner::new()?.destroy(env, project_name).await,
        CloudProvider::Azure => AzureProvisioner::new().destroy(env, project_name).await,
        CloudProvider::Fly => FlyProvisioner::new().destroy(env, project_name).await,
        CloudProvider::Cooper => CooperCloudProvisioner::new().destroy(env, project_name).await,
    }
}
