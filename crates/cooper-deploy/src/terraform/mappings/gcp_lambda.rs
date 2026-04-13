use super::{MappingConfig, ResourceMapping};
use super::gcp_server::GcpServerMapping;
use crate::terraform::hcl_builder::*;
use cooper_codegen::analyzer::*;
use serde_json::json;

/// GCP Serverless mapping: Cloud Functions v2 + Cloud SQL + Memorystore + Pub/Sub + GCS.
/// Database, cache, messaging, and storage mappings reuse the server mode.
pub struct GcpLambdaMapping;

impl ResourceMapping for GcpLambdaMapping {
    fn providers(&self, config: &MappingConfig) -> Vec<TerraformProvider> {
        GcpServerMapping.providers(config)
    }

    fn map_networking(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        GcpServerMapping.map_networking(config)
    }

    fn map_compute(&self, _routes: &[RouteInfo], config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            // GCS bucket for function source
            TerraformResource::new("google_storage_bucket", "function_source")
                .attr("name", format!("{prefix}-function-source"))
                .attr("location", "${var.gcp_region}")
                .attr("force_destroy", true)
                .attr("uniform_bucket_level_access", true),

            // Upload function source zip
            TerraformResource::new("google_storage_bucket_object", "function_source")
                .attr("name", "function-source.zip")
                .attr_ref("bucket", "google_storage_bucket.function_source.name")
                .attr("source", "function-source.zip"),

            // Cloud Functions v2
            TerraformResource::new("google_cloudfunctions2_function", "api")
                .attr("name", format!("{prefix}-api"))
                .attr("location", "${var.gcp_region}")
                .attr_block("build_config", json!({
                    "runtime": "nodejs20",
                    "entry_point": "handler",
                    "source": {
                        "storage_source": {
                            "bucket": format!("${{google_storage_bucket.function_source.name}}"),
                            "object": format!("${{google_storage_bucket_object.function_source.name}}")
                        }
                    }
                }))
                .attr_block("service_config", json!({
                    "available_memory": "512M",
                    "timeout_seconds": 60,
                    "max_instance_count": 100,
                    "min_instance_count": 0,
                    "vpc_connector": format!("${{google_vpc_access_connector.main.id}}"),
                    "environment_variables": {
                        "NODE_ENV": "production",
                        "COOPER_ENV": config.env
                    }
                })),

            // Allow unauthenticated invocations
            TerraformResource::new("google_cloud_run_v2_service_iam_member", "function_public")
                .attr_ref("name", "google_cloudfunctions2_function.api.name")
                .attr_ref("location", "google_cloudfunctions2_function.api.location")
                .attr("role", "roles/run.invoker")
                .attr("member", "allUsers"),
        ]
    }

    fn map_database(&self, db: &DatabaseInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        GcpServerMapping.map_database(db, config)
    }

    fn map_cache(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        GcpServerMapping.map_cache(config)
    }

    fn map_topic(&self, topic: &TopicInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        GcpServerMapping.map_topic(topic, config)
    }

    fn map_queue(&self, queue: &QueueInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        GcpServerMapping.map_queue(queue, config)
    }

    fn map_storage(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        GcpServerMapping.map_storage(config)
    }

    fn map_iam(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        GcpServerMapping.map_iam(config)
    }

    fn variables(&self, config: &MappingConfig) -> Vec<TerraformVariable> {
        GcpServerMapping.variables(config)
    }

    fn outputs(&self, _config: &MappingConfig) -> Vec<TerraformOutput> {
        vec![
            TerraformOutput::new(
                "function_url",
                "google_cloudfunctions2_function.api.url",
                "Cloud Functions URL",
            ),
        ]
    }

    fn extra_blocks(&self, config: &MappingConfig) -> Vec<String> {
        let mut blocks = GcpServerMapping.extra_blocks(config);
        blocks.push(
            "resource \"google_project_service\" \"cloudfunctions\" {\n  service = \"cloudfunctions.googleapis.com\"\n  disable_on_destroy = false\n}".to_string(),
        );
        blocks.push(
            "resource \"google_project_service\" \"cloudbuild\" {\n  service = \"cloudbuild.googleapis.com\"\n  disable_on_destroy = false\n}".to_string(),
        );
        blocks
    }
}
