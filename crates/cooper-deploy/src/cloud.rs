use crate::{CloudProvider, DeployPlan, ResourceChange};
use anyhow::Result;

/// Maps Cooper declarations to cloud-specific resources
pub fn plan_deployment(
    provider: &CloudProvider,
    analysis: &cooper_codegen::analyzer::ProjectAnalysis,
    env: &str,
) -> Result<DeployPlan> {
    let mut plan = DeployPlan {
        creates: Vec::new(),
        updates: Vec::new(),
        deletes: Vec::new(),
        estimated_monthly_cost: 0.0,
    };

    // Map databases
    for db in &analysis.databases {
        let (resource_type, detail, cost) = match provider {
            CloudProvider::Aws => match db.engine.as_str() {
                "postgres" => ("RDS Postgres".into(), "db.t3.medium".into(), 28.0),
                "mysql" => ("RDS MySQL".into(), "db.t3.medium".into(), 28.0),
                "dynamodb" => ("DynamoDB Table".into(), "on-demand".into(), 0.0),
                _ => ("RDS Postgres".into(), "db.t3.medium".into(), 28.0),
            },
            CloudProvider::Gcp => ("Cloud SQL".into(), "db-f1-micro".into(), 10.0),
            CloudProvider::Azure => ("Azure Database".into(), "Basic".into(), 15.0),
            CloudProvider::Fly => ("Fly Postgres".into(), "shared-cpu-1x".into(), 0.0),
        };

        plan.creates.push(ResourceChange {
            resource_type,
            name: format!("{}-{}", env, db.name),
            detail,
            estimated_cost: Some(cost),
        });
        plan.estimated_monthly_cost += cost;
    }

    // Map topics/queues
    for topic in &analysis.topics {
        let (resource_type, cost) = match provider {
            CloudProvider::Aws => ("SNS/SQS".into(), 0.0),
            CloudProvider::Gcp => ("Cloud Pub/Sub".into(), 0.0),
            CloudProvider::Azure => ("Service Bus".into(), 0.0),
            CloudProvider::Fly => ("Upstash".into(), 0.0),
        };

        plan.creates.push(ResourceChange {
            resource_type,
            name: format!("{}-{}", env, topic.name),
            detail: "Standard".into(),
            estimated_cost: Some(cost),
        });
    }

    // Map cache
    let cache_cost = match provider {
        CloudProvider::Aws => 12.0,
        CloudProvider::Gcp => 10.0,
        CloudProvider::Azure => 13.0,
        CloudProvider::Fly => 0.0,
    };

    plan.creates.push(ResourceChange {
        resource_type: match provider {
            CloudProvider::Aws => "ElastiCache".into(),
            CloudProvider::Gcp => "Memorystore".into(),
            CloudProvider::Azure => "Azure Redis".into(),
            CloudProvider::Fly => "Upstash Redis".into(),
        },
        name: format!("{}-cache", env),
        detail: "cache.t3.micro".into(),
        estimated_cost: Some(cache_cost),
    });
    plan.estimated_monthly_cost += cache_cost;

    // Map compute (the app itself)
    let compute_cost = match provider {
        CloudProvider::Aws => 0.0,  // Fargate — pay per use
        CloudProvider::Gcp => 0.0,  // Cloud Run — pay per use
        CloudProvider::Azure => 0.0, // Container Apps — pay per use
        CloudProvider::Fly => 0.0,  // Pay per use
    };

    plan.creates.push(ResourceChange {
        resource_type: match provider {
            CloudProvider::Aws => "ECS Fargate Service".into(),
            CloudProvider::Gcp => "Cloud Run Service".into(),
            CloudProvider::Azure => "Container App".into(),
            CloudProvider::Fly => "Fly Machine".into(),
        },
        name: format!("{}-api", env),
        detail: "auto-scaling".into(),
        estimated_cost: Some(compute_cost),
    });

    Ok(plan)
}
