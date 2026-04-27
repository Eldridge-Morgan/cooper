pub mod executor;
pub mod generator;
pub mod hcl_builder;
pub mod mappings;

use crate::{CloudProvider, ServiceType};
use anyhow::Result;
use cooper_codegen::analyzer::ProjectAnalysis;
use hcl_builder::TerraformConfig;

/// Generate Terraform configuration based on stack analysis.
pub fn generate(
    provider: &CloudProvider,
    service_type: &ServiceType,
    analysis: &ProjectAnalysis,
    env: &str,
    project_name: &str,
) -> Result<TerraformConfig> {
    let mapping = mappings::get_mapping(provider, service_type)?;

    generator::generate_config(
        mapping.as_ref(),
        analysis,
        env,
        project_name,
        provider,
        service_type,
    )
}

/// Apply Terraform configuration (init → plan → apply → outputs).
pub async fn apply(
    tf_dir: &str,
    credentials: &crate::credentials::Credentials,
    env: &str,
    project_name: &str,
) -> Result<crate::DeployResult> {
    executor::execute_terraform(tf_dir, credentials, env, project_name).await
}

/// Destroy Terraform-managed infrastructure.
/// Refreshes state first to catch orphaned resources from cancelled deploys.
pub async fn destroy(
    tf_dir: &str,
    credentials: &crate::credentials::Credentials,
) -> Result<()> {
    executor::terraform_init(tf_dir, credentials).await?;
    executor::terraform_refresh(tf_dir, credentials).await?;
    executor::terraform_destroy(tf_dir, credentials).await
}
