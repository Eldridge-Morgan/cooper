use super::{MappingConfig, ResourceMapping};
use super::azure_server::AzureServerMapping;
use crate::terraform::hcl_builder::*;
use cooper_codegen::analyzer::*;
use serde_json::json;

/// Azure Serverless mapping: Azure Functions + Azure Database + Azure Redis + Service Bus + Blob Storage.
/// Database, cache, messaging, and storage mappings reuse the server mode.
pub struct AzureLambdaMapping;

impl ResourceMapping for AzureLambdaMapping {
    fn providers(&self, config: &MappingConfig) -> Vec<TerraformProvider> {
        AzureServerMapping.providers(config)
    }

    fn map_networking(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        AzureServerMapping.map_networking(config)
    }

    fn map_compute(&self, _routes: &[RouteInfo], config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        // Storage account for the function app (required by Azure Functions)
        let func_storage: String = format!("{}func", prefix)
            .chars()
            .filter(|c| c.is_alphanumeric())
            .take(24)
            .collect::<String>()
            .to_lowercase();

        vec![
            // Storage account for Function App
            TerraformResource::new("azurerm_storage_account", "function")
                .attr("name", func_storage)
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr_ref("location", "azurerm_resource_group.main.location")
                .attr("account_tier", "Standard")
                .attr("account_replication_type", "LRS"),

            // App Service Plan (Consumption)
            TerraformResource::new("azurerm_service_plan", "function")
                .attr("name", format!("{prefix}-plan"))
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr_ref("location", "azurerm_resource_group.main.location")
                .attr("os_type", "Linux")
                .attr("sku_name", "Y1"),

            // Linux Function App
            TerraformResource::new("azurerm_linux_function_app", "api")
                .attr("name", format!("{prefix}-api"))
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr_ref("location", "azurerm_resource_group.main.location")
                .attr_ref("service_plan_id", "azurerm_service_plan.function.id")
                .attr_ref("storage_account_name", "azurerm_storage_account.function.name")
                .attr_ref("storage_account_access_key", "azurerm_storage_account.function.primary_access_key")
                .attr_block("site_config", json!({
                    "application_stack": {
                        "node_version": "20"
                    }
                }))
                .attr_block("app_settings", json!({
                    "NODE_ENV": "production",
                    "COOPER_ENV": config.env,
                    "FUNCTIONS_WORKER_RUNTIME": "node"
                })),
        ]
    }

    fn map_database(&self, db: &DatabaseInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        AzureServerMapping.map_database(db, config)
    }

    fn map_cache(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        AzureServerMapping.map_cache(config)
    }

    fn map_topic(&self, topic: &TopicInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        AzureServerMapping.map_topic(topic, config)
    }

    fn map_queue(&self, queue: &QueueInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        AzureServerMapping.map_queue(queue, config)
    }

    fn map_storage(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        AzureServerMapping.map_storage(config)
    }

    fn map_iam(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        AzureServerMapping.map_iam(config)
    }

    fn variables(&self, config: &MappingConfig) -> Vec<TerraformVariable> {
        AzureServerMapping.variables(config)
    }

    fn outputs(&self, _config: &MappingConfig) -> Vec<TerraformOutput> {
        vec![
            TerraformOutput::new(
                "function_app_url",
                "\"https://${azurerm_linux_function_app.api.default_hostname}\"",
                "Azure Functions URL",
            ),
        ]
    }

    fn extra_blocks(&self, config: &MappingConfig) -> Vec<String> {
        AzureServerMapping.extra_blocks(config)
    }
}
