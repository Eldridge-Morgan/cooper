pub mod aws_server;
pub mod aws_lambda;
pub mod gcp_server;
pub mod gcp_lambda;
pub mod azure_server;
pub mod azure_lambda;

use crate::{CloudProvider, ServiceType};
use super::hcl_builder::*;
use anyhow::Result;
use cooper_codegen::analyzer::*;

/// Configuration passed to every mapping method.
pub struct MappingConfig {
    pub prefix: String,
    pub env: String,
    pub project_name: String,
    pub region: String,
    pub database_names: Vec<String>,
}

/// Trait that each cloud+service mapping implements.
pub trait ResourceMapping {
    /// Generate provider block(s).
    fn providers(&self, config: &MappingConfig) -> Vec<TerraformProvider>;

    /// Generate networking resources (VPC, subnets, etc.).
    fn map_networking(&self, config: &MappingConfig) -> Vec<TerraformResource>;

    /// Generate compute resources from the project's routes.
    fn map_compute(&self, routes: &[RouteInfo], config: &MappingConfig) -> Vec<TerraformResource>;

    /// Generate database resources for a single database declaration.
    fn map_database(&self, db: &DatabaseInfo, config: &MappingConfig) -> Vec<TerraformResource>;

    /// Generate cache resources.
    fn map_cache(&self, config: &MappingConfig) -> Vec<TerraformResource>;

    /// Generate messaging resources for a topic.
    fn map_topic(&self, topic: &TopicInfo, config: &MappingConfig) -> Vec<TerraformResource>;

    /// Generate queue resources.
    fn map_queue(&self, queue: &QueueInfo, config: &MappingConfig) -> Vec<TerraformResource>;

    /// Generate object storage resources.
    fn map_storage(&self, config: &MappingConfig) -> Vec<TerraformResource>;

    /// Generate IAM / role resources needed by compute.
    fn map_iam(&self, config: &MappingConfig) -> Vec<TerraformResource>;

    /// Generate variables for this mapping.
    fn variables(&self, config: &MappingConfig) -> Vec<TerraformVariable>;

    /// Generate outputs for this mapping.
    fn outputs(&self, config: &MappingConfig) -> Vec<TerraformOutput>;

    /// Extra HCL blocks (locals, data sources, random resources, etc.).
    fn extra_blocks(&self, config: &MappingConfig) -> Vec<String> {
        let _ = config;
        Vec::new()
    }
}

/// Get the appropriate mapping for a cloud provider + service type combination.
pub fn get_mapping(
    provider: &CloudProvider,
    service_type: &ServiceType,
) -> Result<Box<dyn ResourceMapping>> {
    match (provider, service_type) {
        (CloudProvider::Aws, ServiceType::Server) => Ok(Box::new(aws_server::AwsServerMapping)),
        (CloudProvider::Aws, ServiceType::Serverless) => {
            Ok(Box::new(aws_lambda::AwsLambdaMapping))
        }
        (CloudProvider::Gcp, ServiceType::Server) => Ok(Box::new(gcp_server::GcpServerMapping)),
        (CloudProvider::Gcp, ServiceType::Serverless) => {
            Ok(Box::new(gcp_lambda::GcpLambdaMapping))
        }
        (CloudProvider::Azure, ServiceType::Server) => {
            Ok(Box::new(azure_server::AzureServerMapping))
        }
        (CloudProvider::Azure, ServiceType::Serverless) => {
            Ok(Box::new(azure_lambda::AzureLambdaMapping))
        }
        (CloudProvider::Fly, ServiceType::Server) => {
            Err(anyhow::anyhow!(
                "Fly.io Terraform deployment is not yet supported. Use `cooper deploy --cloud fly` without Terraform."
            ))
        }
        (CloudProvider::Fly, ServiceType::Serverless) => {
            Err(anyhow::anyhow!(
                "Fly.io does not support serverless deployments."
            ))
        }
    }
}
