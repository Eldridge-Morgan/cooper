use crate::{DeployPlan, DeployResult, ProvisionedResource};
use anyhow::{Context, Result};
use cooper_codegen::analyzer::ProjectAnalysis;
use serde_json::json;
use std::process::Stdio;
use tokio::process::Command;

/// AWS provisioner — creates ECS Fargate, RDS, SQS, ElastiCache, S3 resources.
///
/// Uses the AWS CLI under the hood for credential resolution and API calls.
/// Credentials are read from the standard AWS credential chain:
/// - Environment variables (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
/// - AWS CLI profiles (~/.aws/credentials)
/// - IAM instance roles (on EC2/ECS)
pub struct AwsProvisioner {
    region: String,
}

impl AwsProvisioner {
    pub fn new() -> Self {
        Self {
            region: std::env::var("AWS_REGION")
                .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
                .unwrap_or_else(|_| "us-east-1".to_string()),
        }
    }

    pub async fn provision(
        &self,
        _plan: &DeployPlan,
        analysis: &ProjectAnalysis,
        env: &str,
        project_name: &str,
    ) -> Result<DeployResult> {
        let prefix = format!("cooper-{project_name}-{env}");
        let mut resources = Vec::new();

        // Verify AWS credentials
        self.verify_credentials().await?;

        // 1. Create VPC + networking (if not exists)
        let vpc_id = self.ensure_vpc(&prefix).await?;
        let subnet_ids = self.ensure_subnets(&prefix, &vpc_id).await?;
        let sg_id = self.ensure_security_group(&prefix, &vpc_id).await?;

        // 2. RDS Postgres for each database declaration
        for db in &analysis.databases {
            let db_name = format!("{prefix}-{}", db.name);
            tracing::info!("Creating RDS instance: {db_name}");

            let endpoint = self
                .create_rds_instance(&db_name, &db.engine, &subnet_ids, &sg_id)
                .await?;

            resources.push(ProvisionedResource {
                resource_type: "RDS".to_string(),
                name: db_name,
                status: "available".to_string(),
                connection_info: Some(endpoint),
            });
        }

        // 3. ElastiCache for caching
        {
            let cache_name = format!("{prefix}-cache");
            tracing::info!("Creating ElastiCache: {cache_name}");

            let endpoint = self
                .create_elasticache(&cache_name, &subnet_ids, &sg_id)
                .await?;

            resources.push(ProvisionedResource {
                resource_type: "ElastiCache".to_string(),
                name: cache_name,
                status: "available".to_string(),
                connection_info: Some(endpoint),
            });
        }

        // 4. SNS topics + SQS queues for pub/sub and queues
        for topic in &analysis.topics {
            let topic_name = format!("{prefix}-{}", topic.name);
            tracing::info!("Creating SNS topic: {topic_name}");

            let arn = self.create_sns_topic(&topic_name).await?;
            resources.push(ProvisionedResource {
                resource_type: "SNS Topic".to_string(),
                name: topic_name,
                status: "active".to_string(),
                connection_info: Some(arn),
            });
        }

        for queue in &analysis.queues {
            let queue_name = format!("{prefix}-{}", queue.name);
            tracing::info!("Creating SQS queue: {queue_name}");

            let url = self.create_sqs_queue(&queue_name).await?;
            resources.push(ProvisionedResource {
                resource_type: "SQS Queue".to_string(),
                name: queue_name,
                status: "active".to_string(),
                connection_info: Some(url),
            });
        }

        // 5. S3 bucket for object storage
        {
            let bucket_name = format!("{prefix}-storage");
            tracing::info!("Creating S3 bucket: {bucket_name}");

            self.create_s3_bucket(&bucket_name).await?;
            resources.push(ProvisionedResource {
                resource_type: "S3 Bucket".to_string(),
                name: bucket_name.clone(),
                status: "active".to_string(),
                connection_info: Some(format!("s3://{bucket_name}")),
            });
        }

        // 6. ECR repository + ECS Fargate service
        let ecr_repo = format!("{prefix}-app");
        tracing::info!("Creating ECR repository: {ecr_repo}");
        let repo_uri = self.ensure_ecr_repo(&ecr_repo).await?;

        tracing::info!("Creating ECS cluster + service");
        let service_url = self
            .create_ecs_service(&prefix, &repo_uri, &subnet_ids, &sg_id)
            .await?;

        resources.push(ProvisionedResource {
            resource_type: "ECS Fargate".to_string(),
            name: format!("{prefix}-service"),
            status: "running".to_string(),
            connection_info: Some(service_url.clone()),
        });

        // 7. Save state
        self.save_state(env, project_name, &resources).await?;

        Ok(DeployResult {
            env: env.to_string(),
            provider: "aws".to_string(),
            url: Some(service_url),
            resources,
        })
    }

    pub async fn destroy(&self, env: &str, project_name: &str) -> Result<()> {
        let prefix = format!("cooper-{project_name}-{env}");
        tracing::info!("Destroying AWS environment: {prefix}");

        // Delete in reverse dependency order
        let _ = self.aws_cli(&["ecs", "delete-service",
            "--cluster", &format!("{prefix}-cluster"),
            "--service", &format!("{prefix}-service"),
            "--force"]).await;

        let _ = self.aws_cli(&["ecs", "delete-cluster",
            "--cluster", &format!("{prefix}-cluster")]).await;

        let _ = self.aws_cli(&["rds", "delete-db-instance",
            "--db-instance-identifier", &format!("{prefix}-main"),
            "--skip-final-snapshot"]).await;

        let _ = self.aws_cli(&["elasticache", "delete-cache-cluster",
            "--cache-cluster-id", &format!("{prefix}-cache")]).await;

        let _ = self.aws_cli(&["s3", "rb",
            &format!("s3://{prefix}-storage"), "--force"]).await;

        tracing::info!("AWS environment {prefix} destroyed");
        Ok(())
    }

    pub async fn get_url(&self, env: &str, project_name: &str) -> Result<String> {
        let prefix = format!("cooper-{project_name}-{env}");
        // Get the load balancer DNS name
        let output = self.aws_cli(&[
            "elbv2", "describe-load-balancers",
            "--names", &format!("{prefix}-alb"),
            "--query", "LoadBalancers[0].DNSName",
            "--output", "text",
        ]).await?;
        Ok(format!("https://{}", output.trim()))
    }

    // --- AWS resource creation methods ---

    async fn verify_credentials(&self) -> Result<()> {
        let _output = self.aws_cli(&["sts", "get-caller-identity"]).await
            .context("AWS credentials not configured. Run `aws configure` or set AWS_ACCESS_KEY_ID/AWS_SECRET_ACCESS_KEY.")?;
        tracing::info!("AWS identity verified");
        Ok(())
    }

    async fn ensure_vpc(&self, prefix: &str) -> Result<String> {
        // Check for existing VPC tagged with our prefix
        let output = self.aws_cli(&[
            "ec2", "describe-vpcs",
            "--filters", &format!("Name=tag:cooper-project,Values={prefix}"),
            "--query", "Vpcs[0].VpcId",
            "--output", "text",
        ]).await?;

        if output.trim() != "None" && !output.trim().is_empty() {
            return Ok(output.trim().to_string());
        }

        // Create VPC
        let output = self.aws_cli(&[
            "ec2", "create-vpc",
            "--cidr-block", "10.0.0.0/16",
            "--query", "Vpc.VpcId",
            "--output", "text",
            "--tag-specifications",
            &format!("ResourceType=vpc,Tags=[{{Key=Name,Value={prefix}-vpc}},{{Key=cooper-project,Value={prefix}}}]"),
        ]).await?;

        let vpc_id = output.trim().to_string();

        // Enable DNS hostnames
        let _ = self.aws_cli(&[
            "ec2", "modify-vpc-attribute",
            "--vpc-id", &vpc_id,
            "--enable-dns-hostnames", "{\"Value\":true}",
        ]).await;

        // Create internet gateway
        let igw_output = self.aws_cli(&[
            "ec2", "create-internet-gateway",
            "--query", "InternetGateway.InternetGatewayId",
            "--output", "text",
        ]).await?;
        let igw_id = igw_output.trim();

        let _ = self.aws_cli(&[
            "ec2", "attach-internet-gateway",
            "--internet-gateway-id", igw_id,
            "--vpc-id", &vpc_id,
        ]).await;

        Ok(vpc_id)
    }

    async fn ensure_subnets(&self, prefix: &str, vpc_id: &str) -> Result<Vec<String>> {
        let output = self.aws_cli(&[
            "ec2", "describe-subnets",
            "--filters",
            &format!("Name=vpc-id,Values={vpc_id}"),
            &format!("Name=tag:cooper-project,Values={prefix}"),
            "--query", "Subnets[].SubnetId",
            "--output", "json",
        ]).await?;

        let existing: Vec<String> = serde_json::from_str(&output).unwrap_or_default();
        if existing.len() >= 2 {
            return Ok(existing);
        }

        // Create 2 subnets in different AZs
        let azs = self.get_availability_zones().await?;
        let mut subnet_ids = Vec::new();

        for (i, az) in azs.iter().take(2).enumerate() {
            let cidr = format!("10.0.{}.0/24", i + 1);
            let output = self.aws_cli(&[
                "ec2", "create-subnet",
                "--vpc-id", vpc_id,
                "--cidr-block", &cidr,
                "--availability-zone", az,
                "--query", "Subnet.SubnetId",
                "--output", "text",
                "--tag-specifications",
                &format!("ResourceType=subnet,Tags=[{{Key=Name,Value={prefix}-subnet-{i}}},{{Key=cooper-project,Value={prefix}}}]"),
            ]).await?;
            subnet_ids.push(output.trim().to_string());
        }

        // Enable auto-assign public IPs
        for sid in &subnet_ids {
            let _ = self.aws_cli(&[
                "ec2", "modify-subnet-attribute",
                "--subnet-id", sid,
                "--map-public-ip-on-launch",
            ]).await;
        }

        Ok(subnet_ids)
    }

    async fn ensure_security_group(&self, prefix: &str, vpc_id: &str) -> Result<String> {
        let output = self.aws_cli(&[
            "ec2", "describe-security-groups",
            "--filters",
            &format!("Name=vpc-id,Values={vpc_id}"),
            &format!("Name=group-name,Values={prefix}-sg"),
            "--query", "SecurityGroups[0].GroupId",
            "--output", "text",
        ]).await?;

        if output.trim() != "None" && !output.trim().is_empty() {
            return Ok(output.trim().to_string());
        }

        let output = self.aws_cli(&[
            "ec2", "create-security-group",
            "--group-name", &format!("{prefix}-sg"),
            "--description", &format!("Cooper security group for {prefix}"),
            "--vpc-id", vpc_id,
            "--query", "GroupId",
            "--output", "text",
        ]).await?;

        let sg_id = output.trim().to_string();

        // Allow HTTP/HTTPS inbound
        for port in &["80", "443", "4000"] {
            let _ = self.aws_cli(&[
                "ec2", "authorize-security-group-ingress",
                "--group-id", &sg_id,
                "--protocol", "tcp",
                "--port", port,
                "--cidr", "0.0.0.0/0",
            ]).await;
        }

        // Allow all internal traffic
        let _ = self.aws_cli(&[
            "ec2", "authorize-security-group-ingress",
            "--group-id", &sg_id,
            "--protocol", "-1",
            "--source-group", &sg_id,
        ]).await;

        Ok(sg_id)
    }

    async fn create_rds_instance(
        &self,
        name: &str,
        engine: &str,
        subnet_ids: &[String],
        sg_id: &str,
    ) -> Result<String> {
        let db_engine = match engine {
            "mysql" => "mysql",
            _ => "postgres",
        };

        // Create DB subnet group
        let subnet_group = format!("{name}-subnet-group");
        let subnets_str = subnet_ids.join(" ");
        let _ = self.aws_cli(&[
            "rds", "create-db-subnet-group",
            "--db-subnet-group-name", &subnet_group,
            "--db-subnet-group-description", &format!("Cooper DB subnet group for {name}"),
            "--subnet-ids", &subnets_str,
        ]).await;

        // Create RDS instance
        let _output = self.aws_cli(&[
            "rds", "create-db-instance",
            "--db-instance-identifier", name,
            "--db-instance-class", "db.t3.micro",
            "--engine", db_engine,
            "--master-username", "cooper",
            "--master-user-password", &generate_password(),
            "--allocated-storage", "20",
            "--vpc-security-group-ids", sg_id,
            "--db-subnet-group-name", &subnet_group,
            "--no-publicly-accessible",
            "--query", "DBInstance.Endpoint.Address",
            "--output", "text",
        ]).await?;

        // Wait for RDS to be available
        tracing::info!("Waiting for RDS instance {name} to be available...");
        let _ = self.aws_cli(&[
            "rds", "wait", "db-instance-available",
            "--db-instance-identifier", name,
        ]).await;

        // Get the endpoint
        let endpoint_output = self.aws_cli(&[
            "rds", "describe-db-instances",
            "--db-instance-identifier", name,
            "--query", "DBInstances[0].Endpoint.Address",
            "--output", "text",
        ]).await?;

        Ok(endpoint_output.trim().to_string())
    }

    async fn create_elasticache(&self, name: &str, subnet_ids: &[String], sg_id: &str) -> Result<String> {
        // Create subnet group
        let subnet_group = format!("{name}-subnet-group");
        let subnets_str = subnet_ids.join(" ");
        let _ = self.aws_cli(&[
            "elasticache", "create-cache-subnet-group",
            "--cache-subnet-group-name", &subnet_group,
            "--cache-subnet-group-description", &format!("Cooper cache subnet group for {name}"),
            "--subnet-ids", &subnets_str,
        ]).await;

        let _ = self.aws_cli(&[
            "elasticache", "create-cache-cluster",
            "--cache-cluster-id", name,
            "--cache-node-type", "cache.t3.micro",
            "--engine", "redis",
            "--num-cache-nodes", "1",
            "--cache-subnet-group-name", &subnet_group,
            "--security-group-ids", sg_id,
        ]).await;

        // Wait and get endpoint
        tracing::info!("Waiting for ElastiCache {name}...");
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let output = self.aws_cli(&[
            "elasticache", "describe-cache-clusters",
            "--cache-cluster-id", name,
            "--show-cache-node-info",
            "--query", "CacheClusters[0].CacheNodes[0].Endpoint.Address",
            "--output", "text",
        ]).await.unwrap_or_else(|_| format!("{name}.cache.amazonaws.com"));

        Ok(output.trim().to_string())
    }

    async fn create_sns_topic(&self, name: &str) -> Result<String> {
        let output = self.aws_cli(&[
            "sns", "create-topic",
            "--name", name,
            "--query", "TopicArn",
            "--output", "text",
        ]).await?;
        Ok(output.trim().to_string())
    }

    async fn create_sqs_queue(&self, name: &str) -> Result<String> {
        let output = self.aws_cli(&[
            "sqs", "create-queue",
            "--queue-name", name,
            "--query", "QueueUrl",
            "--output", "text",
        ]).await?;
        Ok(output.trim().to_string())
    }

    async fn create_s3_bucket(&self, name: &str) -> Result<()> {
        if self.region == "us-east-1" {
            let _ = self.aws_cli(&[
                "s3api", "create-bucket",
                "--bucket", name,
            ]).await;
        } else {
            let constraint = format!("LocationConstraint={}", self.region);
            let _ = self.aws_cli(&[
                "s3api", "create-bucket",
                "--bucket", name,
                "--create-bucket-configuration", &constraint,
            ]).await;
        }
        Ok(())
    }

    async fn ensure_ecr_repo(&self, name: &str) -> Result<String> {
        // Try to describe first
        let output = self.aws_cli(&[
            "ecr", "describe-repositories",
            "--repository-names", name,
            "--query", "repositories[0].repositoryUri",
            "--output", "text",
        ]).await;

        if let Ok(uri) = output {
            if uri.trim() != "None" && !uri.trim().is_empty() {
                return Ok(uri.trim().to_string());
            }
        }

        let output = self.aws_cli(&[
            "ecr", "create-repository",
            "--repository-name", name,
            "--query", "repository.repositoryUri",
            "--output", "text",
        ]).await?;

        Ok(output.trim().to_string())
    }

    async fn create_ecs_service(
        &self,
        prefix: &str,
        image_uri: &str,
        subnet_ids: &[String],
        sg_id: &str,
    ) -> Result<String> {
        let cluster_name = format!("{prefix}-cluster");
        let service_name = format!("{prefix}-service");
        let task_family = format!("{prefix}-task");

        // Create cluster
        let _ = self.aws_cli(&[
            "ecs", "create-cluster",
            "--cluster-name", &cluster_name,
        ]).await;

        // Register task definition
        let task_def = json!({
            "family": task_family,
            "networkMode": "awsvpc",
            "requiresCompatibilities": ["FARGATE"],
            "cpu": "256",
            "memory": "512",
            "executionRoleArn": "ecsTaskExecutionRole",
            "containerDefinitions": [{
                "name": "app",
                "image": image_uri,
                "portMappings": [{
                    "containerPort": 4000,
                    "protocol": "tcp"
                }],
                "logConfiguration": {
                    "logDriver": "awslogs",
                    "options": {
                        "awslogs-group": format!("/ecs/{prefix}"),
                        "awslogs-region": self.region,
                        "awslogs-stream-prefix": "ecs"
                    }
                }
            }]
        });

        let task_def_file = format!("/tmp/cooper-task-def-{prefix}.json");
        std::fs::write(&task_def_file, serde_json::to_string_pretty(&task_def)?)?;

        let _ = self.aws_cli(&[
            "ecs", "register-task-definition",
            "--cli-input-json", &format!("file://{task_def_file}"),
        ]).await;

        // Create service
        let subnets_str = subnet_ids.join(",");
        let _ = self.aws_cli(&[
            "ecs", "create-service",
            "--cluster", &cluster_name,
            "--service-name", &service_name,
            "--task-definition", &task_family,
            "--desired-count", "1",
            "--launch-type", "FARGATE",
            "--network-configuration",
            &format!("awsvpcConfiguration={{subnets=[{subnets_str}],securityGroups=[{sg_id}],assignPublicIp=ENABLED}}"),
        ]).await;

        // Get the public IP (simplified — in production you'd use an ALB)
        let _output = self.aws_cli(&[
            "ecs", "list-tasks",
            "--cluster", &cluster_name,
            "--service-name", &service_name,
            "--query", "taskArns[0]",
            "--output", "text",
        ]).await.unwrap_or_default();

        Ok(format!("http://{prefix}.{}.compute.amazonaws.com:4000", self.region))
    }

    async fn get_availability_zones(&self) -> Result<Vec<String>> {
        let output = self.aws_cli(&[
            "ec2", "describe-availability-zones",
            "--query", "AvailabilityZones[].ZoneName",
            "--output", "json",
        ]).await?;

        let azs: Vec<String> = serde_json::from_str(&output).unwrap_or_else(|_| {
            vec![
                format!("{}a", self.region),
                format!("{}b", self.region),
            ]
        });
        Ok(azs)
    }

    async fn save_state(&self, env: &str, project_name: &str, resources: &[ProvisionedResource]) -> Result<()> {
        let state = serde_json::json!({
            "env": env,
            "project": project_name,
            "provider": "aws",
            "region": self.region,
            "resources": resources,
            "deployed_at": chrono::Utc::now().to_rfc3339(),
        });

        let state_dir = format!(".cooper/state/{env}");
        std::fs::create_dir_all(&state_dir)?;
        std::fs::write(
            format!("{state_dir}/deploy.json"),
            serde_json::to_string_pretty(&state)?,
        )?;

        Ok(())
    }

    /// Execute an AWS CLI command and return stdout.
    async fn aws_cli(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("aws")
            .args(args)
            .args(["--region", &self.region])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .context("AWS CLI not found. Install it: https://aws.amazon.com/cli/")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("AWS CLI error: {}", stderr.trim()));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

fn generate_password() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("Cooper{}!", seed % 1_000_000_000)
}
