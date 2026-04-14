use super::hcl_builder::TerraformConfig;
use super::mappings::{MappingConfig, ResourceMapping};
use crate::{CloudProvider, ServiceType};
use anyhow::Result;
use cooper_codegen::analyzer::ProjectAnalysis;

/// Generate a complete TerraformConfig from a mapping, analysis, and context.
pub fn generate_config(
    mapping: &dyn ResourceMapping,
    analysis: &ProjectAnalysis,
    env: &str,
    project_name: &str,
    provider: &CloudProvider,
    service_type: &ServiceType,
) -> Result<TerraformConfig> {
    let region = default_region(provider);
    let prefix = format!(
        "cooper-{}-{}",
        sanitize_name(project_name),
        sanitize_name(env)
    );

    let config = MappingConfig {
        prefix,
        env: env.to_string(),
        project_name: project_name.to_string(),
        region,
    };

    let mut tf = TerraformConfig::new();

    // Providers
    tf.providers = mapping.providers(&config);

    // Variables
    tf.variables = mapping.variables(&config);

    // Extra blocks (locals, API enablement, etc.)
    tf.extra_blocks = mapping.extra_blocks(&config);

    // Networking
    tf.resources.extend(mapping.map_networking(&config));

    // IAM
    tf.resources.extend(mapping.map_iam(&config));

    // Databases (deduplicate by name — multiple services may reference the same DB)
    let mut seen_dbs = std::collections::HashSet::new();
    for db in &analysis.databases {
        if seen_dbs.insert(db.name.clone()) {
            tf.resources.extend(mapping.map_database(db, &config));
        }
    }

    // Cache (always generate — Cooper projects use it for sessions, rate limiting, etc.)
    tf.resources.extend(mapping.map_cache(&config));

    // Topics
    for topic in &analysis.topics {
        tf.resources.extend(mapping.map_topic(topic, &config));
    }

    // Queues
    for queue in &analysis.queues {
        tf.resources.extend(mapping.map_queue(queue, &config));
    }

    // Storage
    tf.resources.extend(mapping.map_storage(&config));

    // Compute (depends on everything above)
    tf.resources
        .extend(mapping.map_compute(&analysis.routes, &config));

    // Outputs
    tf.outputs = mapping.outputs(&config);

    // Add DB connection string outputs (deduplicated)
    let mut seen_db_outputs = std::collections::HashSet::new();
    for db in &analysis.databases {
        if !seen_db_outputs.insert(db.name.clone()) {
            continue;
        }
        let db_name = &db.name;
        match provider {
            CloudProvider::Aws => {
                tf.outputs.push(super::hcl_builder::TerraformOutput::new(
                    &format!("db_{db_name}_endpoint"),
                    &format!("aws_db_instance.{db_name}.endpoint"),
                    &format!("RDS endpoint for {db_name}"),
                ).sensitive());
                tf.outputs.push(super::hcl_builder::TerraformOutput::new(
                    &format!("db_{db_name}_password"),
                    &format!("random_password.db_{db_name}.result"),
                    &format!("Database password for {db_name}"),
                ).sensitive());
            }
            CloudProvider::Gcp => {
                tf.outputs.push(super::hcl_builder::TerraformOutput::new(
                    &format!("db_{db_name}_connection_name"),
                    &format!("google_sql_database_instance.{db_name}.connection_name"),
                    &format!("Cloud SQL connection name for {db_name}"),
                ));
            }
            CloudProvider::Azure => {
                tf.outputs.push(super::hcl_builder::TerraformOutput::new(
                    &format!("db_{db_name}_fqdn"),
                    &format!("azurerm_postgresql_flexible_server.{db_name}.fqdn"),
                    &format!("Azure DB hostname for {db_name}"),
                ));
            }
            CloudProvider::Fly => {}
        }
    }

    // Add summary comment showing what was detected
    let summary = format!(
        "# Cooper deploy: {} / {} / {}\n# Detected: {} routes, {} databases, {} topics, {} queues, {} crons\n",
        env,
        provider_name(provider),
        service_type.display_name(),
        analysis.routes.len(),
        analysis.databases.len(),
        analysis.topics.len(),
        analysis.queues.len(),
        analysis.crons.len(),
    );
    tf.extra_blocks.insert(0, summary);

    Ok(tf)
}

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect::<String>()
        .to_lowercase()
}

fn default_region(provider: &CloudProvider) -> String {
    match provider {
        CloudProvider::Aws => {
            std::env::var("AWS_REGION")
                .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
                .unwrap_or_else(|_| "us-east-1".to_string())
        }
        CloudProvider::Gcp => {
            std::env::var("GOOGLE_CLOUD_REGION")
                .unwrap_or_else(|_| "us-central1".to_string())
        }
        CloudProvider::Azure => {
            std::env::var("AZURE_LOCATION")
                .unwrap_or_else(|_| "eastus".to_string())
        }
        CloudProvider::Fly => {
            std::env::var("FLY_REGION")
                .unwrap_or_else(|_| "iad".to_string())
        }
    }
}

fn provider_name(provider: &CloudProvider) -> &str {
    match provider {
        CloudProvider::Aws => "AWS",
        CloudProvider::Gcp => "GCP",
        CloudProvider::Azure => "Azure",
        CloudProvider::Fly => "Fly.io",
    }
}

/// Generate the minimum IAM policy required for this specific project deployment.
pub fn required_iam_policy(
    provider: &CloudProvider,
    service_type: &ServiceType,
    analysis: &ProjectAnalysis,
) -> Option<String> {
    match provider {
        CloudProvider::Aws => Some(aws_iam_policy(service_type, analysis)),
        _ => None, // GCP/Azure use different auth models
    }
}

fn aws_iam_policy(service_type: &ServiceType, analysis: &ProjectAnalysis) -> String {
    let mut actions: Vec<&str> = Vec::new();

    // Always needed: networking + logs + IAM roles
    actions.extend_from_slice(&[
        "ec2:*",
        "logs:*",
        "iam:CreateRole",
        "iam:DeleteRole",
        "iam:GetRole",
        "iam:PassRole",
        "iam:AttachRolePolicy",
        "iam:DetachRolePolicy",
        "iam:ListAttachedRolePolicies",
        "iam:ListRolePolicies",
        "iam:ListInstanceProfilesForRole",
        "iam:TagRole",
        "iam:UntagRole",
    ]);

    // Compute
    match service_type {
        ServiceType::Server => {
            actions.push("ecs:*");
            actions.push("ecr:*");
        }
        ServiceType::Serverless => {
            actions.push("lambda:*");
            actions.push("apigateway:*");
        }
    }

    // Conditional on analysis
    if !analysis.databases.is_empty() {
        actions.push("rds:*");
    }
    if !analysis.topics.is_empty() {
        actions.push("sns:*");
    }
    if !analysis.queues.is_empty() {
        actions.push("sqs:*");
    }

    // Storage + cache are always generated
    actions.push("s3:*");
    actions.push("elasticache:*");

    // Build JSON policy
    let actions_json: Vec<String> = actions.iter().map(|a| format!("        \"{}\"", a)).collect();
    format!(
        r#"{{
    "Version": "2012-10-17",
    "Statement": [
      {{
        "Effect": "Allow",
        "Action": [
{}
        ],
        "Resource": "*"
      }}
    ]
  }}"#,
        actions_json.join(",\n")
    )
}
