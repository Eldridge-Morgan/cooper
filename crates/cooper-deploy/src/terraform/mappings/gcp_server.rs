use super::{MappingConfig, ResourceMapping};
use crate::terraform::hcl_builder::*;
use cooper_codegen::analyzer::*;
use serde_json::json;
use std::collections::BTreeMap;

/// GCP Server mapping: Cloud Run + Cloud SQL + Memorystore + Pub/Sub + GCS.
pub struct GcpServerMapping;

impl ResourceMapping for GcpServerMapping {
    fn providers(&self, _config: &MappingConfig) -> Vec<TerraformProvider> {
        vec![
            TerraformProvider {
                name: "google".to_string(),
                source: "hashicorp/google".to_string(),
                version: "5.0".to_string(),
                config: BTreeMap::from([
                    ("project".to_string(), json!("${var.gcp_project_id}")),
                    ("region".to_string(), json!("${var.gcp_region}")),
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
            TerraformResource::new("google_compute_network", "main")
                .attr("name", format!("{prefix}-network"))
                .attr("auto_create_subnetworks", false),

            TerraformResource::new("google_compute_subnetwork", "main")
                .attr("name", format!("{prefix}-subnet"))
                .attr_ref("network", "google_compute_network.main.id")
                .attr("ip_cidr_range", "10.0.0.0/24")
                .attr("region", "${var.gcp_region}"),

            // VPC Connector for Cloud Run → Cloud SQL/Memorystore
            TerraformResource::new("google_vpc_access_connector", "main")
                .attr("name", format!("{prefix}-connector"))
                .attr("region", "${var.gcp_region}")
                .attr("ip_cidr_range", "10.8.0.0/28")
                .attr_ref("network", "google_compute_network.main.id"),
        ]
    }

    fn map_compute(&self, _routes: &[RouteInfo], config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            TerraformResource::new("google_cloud_run_v2_service", "app")
                .attr("name", format!("{prefix}-app"))
                .attr("location", "${var.gcp_region}")
                .attr_block("template", json!({
                    "containers": [{
                        "image": format!("gcr.io/${{var.gcp_project_id}}/{prefix}-app:latest"),
                        "ports": [{"container_port": 4000}],
                        "resources": {
                            "limits": {
                                "cpu": "1",
                                "memory": "512Mi"
                            }
                        }
                    }],
                    "vpc_access": {
                        "connector": format!("${{google_vpc_access_connector.main.id}}")
                    },
                    "scaling": {
                        "min_instance_count": 0,
                        "max_instance_count": 10
                    }
                })),

            // Allow unauthenticated access
            TerraformResource::new("google_cloud_run_v2_service_iam_member", "public")
                .attr_ref("name", "google_cloud_run_v2_service.app.name")
                .attr_ref("location", "google_cloud_run_v2_service.app.location")
                .attr("role", "roles/run.invoker")
                .attr("member", "allUsers"),
        ]
    }

    fn map_database(&self, db: &DatabaseInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        let db_name = &db.name;
        let db_version = match db.engine.as_str() {
            "mysql" => "MYSQL_8_0",
            _ => "POSTGRES_16",
        };

        vec![
            TerraformResource::new("random_password", &format!("db_{db_name}"))
                .attr("length", 24)
                .attr("special", false),

            TerraformResource::new("google_sql_database_instance", db_name)
                .attr("name", format!("{prefix}-{db_name}"))
                .attr("database_version", db_version)
                .attr("region", "${var.gcp_region}")
                .attr("deletion_protection", false)
                .attr_block("settings", json!({
                    "tier": "db-f1-micro",
                    "ip_configuration": {
                        "ipv4_enabled": true,
                        "private_network": format!("${{google_compute_network.main.id}}")
                    }
                })),

            TerraformResource::new("google_sql_database", db_name)
                .attr("name", format!("cooper_{db_name}"))
                .attr_ref("instance", &format!("google_sql_database_instance.{db_name}.name")),

            TerraformResource::new("google_sql_user", db_name)
                .attr("name", "cooper")
                .attr_ref("instance", &format!("google_sql_database_instance.{db_name}.name"))
                .attr("password", format!("${{random_password.db_{db_name}.result}}")),
        ]
    }

    fn map_cache(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            TerraformResource::new("google_redis_instance", "cache")
                .attr("name", format!("{prefix}-cache"))
                .attr("tier", "BASIC")
                .attr("memory_size_gb", 1)
                .attr("region", "${var.gcp_region}")
                .attr_ref("authorized_network", "google_compute_network.main.id"),
        ]
    }

    fn map_topic(&self, topic: &TopicInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        let name = &topic.name;
        vec![
            TerraformResource::new("google_pubsub_topic", name)
                .attr("name", format!("{prefix}-{name}")),
        ]
    }

    fn map_queue(&self, queue: &QueueInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        let name = &queue.name;
        // GCP uses Cloud Tasks for queues
        vec![
            TerraformResource::new("google_cloud_tasks_queue", name)
                .attr("name", format!("{prefix}-{name}"))
                .attr("location", "${var.gcp_region}"),
        ]
    }

    fn map_storage(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            TerraformResource::new("google_storage_bucket", "storage")
                .attr("name", format!("{prefix}-storage"))
                .attr("location", "${var.gcp_region}")
                .attr("force_destroy", true)
                .attr("uniform_bucket_level_access", true),
        ]
    }

    fn map_iam(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            TerraformResource::new("google_service_account", "app")
                .attr("account_id", format!("{prefix}-sa"))
                .attr("display_name", format!("Cooper service account for {prefix}")),

            TerraformResource::new("google_project_iam_member", "cloudsql")
                .attr("project", "${var.gcp_project_id}")
                .attr("role", "roles/cloudsql.client")
                .attr("member", "serviceAccount:${google_service_account.app.email}"),
        ]
    }

    fn variables(&self, config: &MappingConfig) -> Vec<TerraformVariable> {
        vec![
            TerraformVariable::new("gcp_project_id", "string", "GCP project ID"),
            TerraformVariable::new("gcp_region", "string", "GCP region")
                .with_default("us-central1"),
            TerraformVariable::new("environment", "string", "Environment name")
                .with_default(config.env.as_str()),
            TerraformVariable::new("project_name", "string", "Cooper project name")
                .with_default(config.project_name.as_str()),
        ]
    }

    fn outputs(&self, _config: &MappingConfig) -> Vec<TerraformOutput> {
        vec![
            TerraformOutput::new(
                "service_url",
                "google_cloud_run_v2_service.app.uri",
                "Cloud Run service URL",
            ),
        ]
    }

    fn extra_blocks(&self, _config: &MappingConfig) -> Vec<String> {
        vec![
            // Enable required APIs
            "resource \"google_project_service\" \"run\" {\n  service = \"run.googleapis.com\"\n  disable_on_destroy = false\n}".to_string(),
            "resource \"google_project_service\" \"sqladmin\" {\n  service = \"sqladmin.googleapis.com\"\n  disable_on_destroy = false\n}".to_string(),
            "resource \"google_project_service\" \"vpcaccess\" {\n  service = \"vpcaccess.googleapis.com\"\n  disable_on_destroy = false\n}".to_string(),
        ]
    }
}
