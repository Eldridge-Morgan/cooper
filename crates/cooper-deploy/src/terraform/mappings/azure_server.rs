use super::{MappingConfig, ResourceMapping};
use crate::terraform::hcl_builder::*;
use cooper_codegen::analyzer::*;
use serde_json::json;
use std::collections::BTreeMap;

/// Azure Server mapping: Container Apps + Azure Database + Azure Redis + Service Bus + Blob Storage.
pub struct AzureServerMapping;

impl ResourceMapping for AzureServerMapping {
    fn providers(&self, _config: &MappingConfig) -> Vec<TerraformProvider> {
        vec![
            TerraformProvider {
                name: "azurerm".to_string(),
                source: "hashicorp/azurerm".to_string(),
                version: "3.0".to_string(),
                config: BTreeMap::from([
                    ("features".to_string(), json!({})),
                ]),
            },
            TerraformProvider {
                name: "random".to_string(),
                source: "hashicorp/random".to_string(),
                version: "3.0".to_string(),
                config: BTreeMap::new(),
            },
        ]
    }

    fn map_networking(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            // Resource Group
            TerraformResource::new("azurerm_resource_group", "main")
                .attr("name", format!("{prefix}-rg"))
                .attr("location", "${var.azure_location}"),

            // Virtual Network
            TerraformResource::new("azurerm_virtual_network", "main")
                .attr("name", format!("{prefix}-vnet"))
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr_ref("location", "azurerm_resource_group.main.location")
                .attr("address_space", json!(["10.0.0.0/16"])),

            // Subnet for Container Apps
            TerraformResource::new("azurerm_subnet", "app")
                .attr("name", format!("{prefix}-app-subnet"))
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr_ref("virtual_network_name", "azurerm_virtual_network.main.name")
                .attr("address_prefixes", json!(["10.0.1.0/24"])),

            // Subnet for databases/services
            TerraformResource::new("azurerm_subnet", "services")
                .attr("name", format!("{prefix}-services-subnet"))
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr_ref("virtual_network_name", "azurerm_virtual_network.main.name")
                .attr("address_prefixes", json!(["10.0.2.0/24"])),
        ]
    }

    fn map_compute(&self, _routes: &[RouteInfo], config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            // Log Analytics Workspace
            TerraformResource::new("azurerm_log_analytics_workspace", "main")
                .attr("name", format!("{prefix}-logs"))
                .attr_ref("location", "azurerm_resource_group.main.location")
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr("sku", "PerGB2018")
                .attr("retention_in_days", 30),

            // Container Apps Environment
            TerraformResource::new("azurerm_container_app_environment", "main")
                .attr("name", format!("{prefix}-env"))
                .attr_ref("location", "azurerm_resource_group.main.location")
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr_ref("log_analytics_workspace_id", "azurerm_log_analytics_workspace.main.id"),

            // Container App
            TerraformResource::new("azurerm_container_app", "app")
                .attr("name", format!("{prefix}-app"))
                .attr_ref("container_app_environment_id", "azurerm_container_app_environment.main.id")
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr("revision_mode", "Single")
                .attr_block("template", json!({
                    "container": [{
                        "name": "app",
                        "image": "mcr.microsoft.com/azuredocs/containerapps-helloworld:latest",
                        "cpu": 0.25,
                        "memory": "0.5Gi"
                    }],
                    "min_replicas": 0,
                    "max_replicas": 10
                }))
                .attr_block("ingress", json!({
                    "external_enabled": true,
                    "target_port": 4000,
                    "traffic_weight": [{
                        "percentage": 100,
                        "latest_revision": true
                    }]
                })),
        ]
    }

    fn map_database(&self, db: &DatabaseInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        let db_name = &db.name;

        vec![
            TerraformResource::new("random_password", &format!("db_{db_name}"))
                .attr("length", 24)
                .attr("special", false),

            TerraformResource::new("azurerm_postgresql_flexible_server", db_name)
                .attr("name", format!("{prefix}-{db_name}"))
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr_ref("location", "azurerm_resource_group.main.location")
                .attr("version", "15")
                .attr("sku_name", "B_Standard_B1ms")
                .attr("storage_mb", 32768)
                .attr("administrator_login", "cooper")
                .attr("administrator_password", format!("${{random_password.db_{db_name}.result}}"))
                .attr("zone", "1"),

            TerraformResource::new("azurerm_postgresql_flexible_server_database", db_name)
                .attr("name", format!("cooper_{db_name}"))
                .attr_ref("server_id", &format!("azurerm_postgresql_flexible_server.{db_name}.id")),

            // Firewall rule to allow Azure services
            TerraformResource::new("azurerm_postgresql_flexible_server_firewall_rule", db_name)
                .attr("name", "allow-azure-services")
                .attr_ref("server_id", &format!("azurerm_postgresql_flexible_server.{db_name}.id"))
                .attr("start_ip_address", "0.0.0.0")
                .attr("end_ip_address", "0.0.0.0"),
        ]
    }

    fn map_cache(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            TerraformResource::new("azurerm_redis_cache", "cache")
                .attr("name", format!("{prefix}-cache"))
                .attr_ref("location", "azurerm_resource_group.main.location")
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr("capacity", 0)
                .attr("family", "C")
                .attr("sku_name", "Basic")
                .attr("minimum_tls_version", "1.2"),
        ]
    }

    fn map_topic(&self, topic: &TopicInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        let name = &topic.name;
        vec![
            TerraformResource::new("azurerm_servicebus_queue", name)
                .attr("name", format!("{prefix}-{name}"))
                .attr_ref("namespace_id", "azurerm_servicebus_namespace.main.id"),
        ]
    }

    fn map_queue(&self, queue: &QueueInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        let name = &queue.name;
        vec![
            TerraformResource::new("azurerm_servicebus_queue", &format!("queue_{name}"))
                .attr("name", format!("{prefix}-queue-{name}"))
                .attr_ref("namespace_id", "azurerm_servicebus_namespace.main.id"),
        ]
    }

    fn map_storage(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        // Azure storage names: lowercase, no hyphens, 3-24 chars
        let storage_name: String = prefix
            .chars()
            .filter(|c| c.is_alphanumeric())
            .take(24)
            .collect::<String>()
            .to_lowercase();
        vec![
            TerraformResource::new("azurerm_storage_account", "storage")
                .attr("name", storage_name)
                .attr_ref("resource_group_name", "azurerm_resource_group.main.name")
                .attr_ref("location", "azurerm_resource_group.main.location")
                .attr("account_tier", "Standard")
                .attr("account_replication_type", "LRS"),

            TerraformResource::new("azurerm_storage_container", "storage")
                .attr("name", "cooper-storage")
                .attr_ref("storage_account_name", "azurerm_storage_account.storage.name")
                .attr("container_access_type", "private"),
        ]
    }

    fn map_iam(&self, _config: &MappingConfig) -> Vec<TerraformResource> {
        // Azure RBAC is handled by managed identities with Container Apps
        Vec::new()
    }

    fn variables(&self, config: &MappingConfig) -> Vec<TerraformVariable> {
        vec![
            TerraformVariable::new("azure_location", "string", "Azure region")
                .with_default("eastus"),
            TerraformVariable::new("environment", "string", "Environment name")
                .with_default(config.env.as_str()),
            TerraformVariable::new("project_name", "string", "Cooper project name")
                .with_default(config.project_name.as_str()),
        ]
    }

    fn outputs(&self, _config: &MappingConfig) -> Vec<TerraformOutput> {
        vec![
            TerraformOutput::new(
                "app_url",
                "\"https://${azurerm_container_app.app.latest_revision_fqdn}\"",
                "Container App URL",
            ),
        ]
    }

    fn extra_blocks(&self, config: &MappingConfig) -> Vec<String> {
        let prefix = &config.prefix;
        vec![
            // Service Bus Namespace (needed by topics and queues)
            format!(
                "resource \"azurerm_servicebus_namespace\" \"main\" {{\n  \
                 name                = \"{prefix}-bus\"\n  \
                 location            = azurerm_resource_group.main.location\n  \
                 resource_group_name = azurerm_resource_group.main.name\n  \
                 sku                 = \"Basic\"\n}}"
            ),
        ]
    }
}
