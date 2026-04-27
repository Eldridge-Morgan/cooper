use super::{MappingConfig, ResourceMapping};
use crate::terraform::hcl_builder::*;
use cooper_codegen::analyzer::*;
use serde_json::json;
use std::collections::BTreeMap;

pub struct AwsServerMapping;

impl ResourceMapping for AwsServerMapping {
    fn providers(&self, _config: &MappingConfig) -> Vec<TerraformProvider> {
        vec![
            TerraformProvider {
                name: "aws".to_string(),
                source: "hashicorp/aws".to_string(),
                version: "5.0".to_string(),
                config: BTreeMap::from([
                    ("region".to_string(), json!(format!("${{var.aws_region}}"))),
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
            // VPC
            TerraformResource::new("aws_vpc", "main")
                .attr("cidr_block", "10.0.0.0/16")
                .attr("enable_dns_hostnames", true)
                .attr("enable_dns_support", true)
                .attr_map("tags", json!({"Name": format!("{prefix}-vpc"), "ManagedBy": "cooper"})),

            // Internet Gateway
            TerraformResource::new("aws_internet_gateway", "main")
                .attr_ref("vpc_id", "aws_vpc.main.id")
                .attr_map("tags", json!({"Name": format!("{prefix}-igw")})),

            // Public Subnet A
            TerraformResource::new("aws_subnet", "public_a")
                .attr_ref("vpc_id", "aws_vpc.main.id")
                .attr("cidr_block", "10.0.1.0/24")
                .attr_ref("availability_zone", "\"${var.aws_region}a\"")
                .attr("map_public_ip_on_launch", true)
                .attr_map("tags", json!({"Name": format!("{prefix}-public-a")})),

            // Public Subnet B
            TerraformResource::new("aws_subnet", "public_b")
                .attr_ref("vpc_id", "aws_vpc.main.id")
                .attr("cidr_block", "10.0.2.0/24")
                .attr_ref("availability_zone", "\"${var.aws_region}b\"")
                .attr("map_public_ip_on_launch", true)
                .attr_map("tags", json!({"Name": format!("{prefix}-public-b")})),

            // Route Table
            TerraformResource::new("aws_route_table", "public")
                .attr_ref("vpc_id", "aws_vpc.main.id")
                .attr_block("route", json!({
                    "cidr_block": "0.0.0.0/0",
                    "gateway_id": "${aws_internet_gateway.main.id}"
                }))
                .attr_map("tags", json!({"Name": format!("{prefix}-public-rt")})),

            // Route Table Associations
            TerraformResource::new("aws_route_table_association", "public_a")
                .attr_ref("subnet_id", "aws_subnet.public_a.id")
                .attr_ref("route_table_id", "aws_route_table.public.id"),

            TerraformResource::new("aws_route_table_association", "public_b")
                .attr_ref("subnet_id", "aws_subnet.public_b.id")
                .attr_ref("route_table_id", "aws_route_table.public.id"),

            // Security Group
            TerraformResource::new("aws_security_group", "app")
                .attr("name", format!("{prefix}-sg"))
                .attr("description", format!("Cooper security group for {prefix}"))
                .attr_ref("vpc_id", "aws_vpc.main.id")
                .attr_map("tags", json!({"Name": format!("{prefix}-sg")})),

            // Ingress rules (separate resources to support multiple ports)
            TerraformResource::new("aws_vpc_security_group_ingress_rule", "http")
                .attr_ref("security_group_id", "aws_security_group.app.id")
                .attr("cidr_ipv4", "0.0.0.0/0")
                .attr("from_port", 80)
                .attr("to_port", 80)
                .attr("ip_protocol", "tcp"),

            TerraformResource::new("aws_vpc_security_group_ingress_rule", "https")
                .attr_ref("security_group_id", "aws_security_group.app.id")
                .attr("cidr_ipv4", "0.0.0.0/0")
                .attr("from_port", 443)
                .attr("to_port", 443)
                .attr("ip_protocol", "tcp"),

            TerraformResource::new("aws_vpc_security_group_ingress_rule", "app")
                .attr_ref("security_group_id", "aws_security_group.app.id")
                .attr("cidr_ipv4", "0.0.0.0/0")
                .attr("from_port", 4000)
                .attr("to_port", 4000)
                .attr("ip_protocol", "tcp"),

            // Allow DB traffic within the security group (ECS → RDS)
            TerraformResource::new("aws_vpc_security_group_ingress_rule", "postgres")
                .attr_ref("security_group_id", "aws_security_group.app.id")
                .attr_ref("referenced_security_group_id", "aws_security_group.app.id")
                .attr("from_port", 5432)
                .attr("to_port", 5432)
                .attr("ip_protocol", "tcp"),

            TerraformResource::new("aws_vpc_security_group_ingress_rule", "mysql")
                .attr_ref("security_group_id", "aws_security_group.app.id")
                .attr_ref("referenced_security_group_id", "aws_security_group.app.id")
                .attr("from_port", 3306)
                .attr("to_port", 3306)
                .attr("ip_protocol", "tcp"),

            // Allow Redis traffic within the security group (ECS → ElastiCache)
            TerraformResource::new("aws_vpc_security_group_ingress_rule", "redis")
                .attr_ref("security_group_id", "aws_security_group.app.id")
                .attr_ref("referenced_security_group_id", "aws_security_group.app.id")
                .attr("from_port", 6379)
                .attr("to_port", 6379)
                .attr("ip_protocol", "tcp"),

            TerraformResource::new("aws_vpc_security_group_egress_rule", "all")
                .attr_ref("security_group_id", "aws_security_group.app.id")
                .attr("cidr_ipv4", "0.0.0.0/0")
                .attr("ip_protocol", "-1"),
        ]
    }

    fn map_compute(&self, _routes: &[RouteInfo], config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            // Random SECRET for app (used for password hashing, etc.)
            TerraformResource::new("random_password", "secret")
                .attr("length", 32)
                .attr("special", true),

            // ECR Repository
            TerraformResource::new("aws_ecr_repository", "app")
                .attr("name", format!("{prefix}-app"))
                .attr("force_delete", true)
                .attr_map("tags", json!({"ManagedBy": "cooper"})),

            // CloudWatch Log Group
            TerraformResource::new("aws_cloudwatch_log_group", "app")
                .attr("name", format!("/ecs/{prefix}"))
                .attr("retention_in_days", 14),

            // ECS Cluster
            TerraformResource::new("aws_ecs_cluster", "main")
                .attr("name", format!("{prefix}-cluster"))
                .attr_block("setting", json!({
                    "name": "containerInsights",
                    "value": "enabled"
                })),

            // ECS Task Definition
            TerraformResource::new("aws_ecs_task_definition", "app")
                .attr("family", format!("{prefix}-task"))
                .attr("network_mode", "awsvpc")
                .attr("requires_compatibilities", json!(["FARGATE"]))
                .attr("cpu", "256")
                .attr("memory", "512")
                .attr_ref("execution_role_arn", "aws_iam_role.ecs_execution.arn")
                .attr_ref("task_role_arn", "aws_iam_role.ecs_task.arn")
                .attr("container_definitions", container_definitions_ref(prefix, &config.database_names)),

            // ALB
            TerraformResource::new("aws_lb", "app")
                .attr("name", format!("{prefix}-alb"))
                .attr("internal", false)
                .attr("load_balancer_type", "application")
                .attr("security_groups", json!([format!("${{aws_security_group.app.id}}")]))
                .attr("subnets", json!(["${aws_subnet.public_a.id}", "${aws_subnet.public_b.id}"]))
                .attr_map("tags", json!({"Name": format!("{prefix}-alb"), "ManagedBy": "cooper"})),

            TerraformResource::new("aws_lb_target_group", "app")
                .attr("name", format!("{prefix}-tg"))
                .attr("port", 4000)
                .attr("protocol", "HTTP")
                .attr("target_type", "ip")
                .attr_ref("vpc_id", "aws_vpc.main.id")
                .attr_block("health_check", json!({
                    "path": "/_cooper/health",
                    "port": "4000",
                    "protocol": "HTTP",
                    "healthy_threshold": 2,
                    "unhealthy_threshold": 3,
                    "timeout": 5,
                    "interval": 30
                })),

            TerraformResource::new("aws_lb_listener", "app")
                .attr_ref("load_balancer_arn", "aws_lb.app.arn")
                .attr("port", 80)
                .attr("protocol", "HTTP")
                .attr_block("default_action", json!({
                    "type": "forward",
                    "target_group_arn": "${aws_lb_target_group.app.arn}"
                })),

            // ECS Service
            TerraformResource::new("aws_ecs_service", "app")
                .attr("name", format!("{prefix}-service"))
                .attr_ref("cluster", "aws_ecs_cluster.main.id")
                .attr_ref("task_definition", "aws_ecs_task_definition.app.arn")
                .attr("desired_count", 1)
                .attr("launch_type", "FARGATE")
                .attr("force_new_deployment", true)
                .attr("health_check_grace_period_seconds", 60)
                .attr_block("network_configuration", json!({
                    "subnets": ["${aws_subnet.public_a.id}", "${aws_subnet.public_b.id}"],
                    "security_groups": ["${aws_security_group.app.id}"],
                    "assign_public_ip": true
                }))
                .attr_block("load_balancer", json!({
                    "target_group_arn": "${aws_lb_target_group.app.arn}",
                    "container_name": "app",
                    "container_port": 4000
                }))
                .depends_on(&["aws_lb_listener.app"]),
        ]
    }

    fn map_database(&self, db: &DatabaseInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        let db_name = &db.name;
        let engine = match db.engine.as_str() {
            "mysql" => "mysql",
            _ => "postgres",
        };
        let engine_version = match db.engine.as_str() {
            "mysql" => "8.0",
            _ => "16",
        };

        vec![
            // Random password
            TerraformResource::new("random_password", &format!("db_{db_name}"))
                .attr("length", 24)
                .attr("special", false),

            // DB Subnet Group
            TerraformResource::new("aws_db_subnet_group", db_name)
                .attr("name", format!("{prefix}-{db_name}-subnet-group"))
                .attr("subnet_ids", json!(["${aws_subnet.public_a.id}", "${aws_subnet.public_b.id}"]))
                .attr_map("tags", json!({"Name": format!("{prefix}-{db_name}-subnet-group")})),

            // RDS Instance
            TerraformResource::new("aws_db_instance", db_name)
                .attr("identifier", format!("{prefix}-{db_name}"))
                .attr("engine", engine)
                .attr("engine_version", engine_version)
                .attr("instance_class", "db.t3.micro")
                .attr("allocated_storage", 20)
                .attr("db_name", "cooper")
                .attr("username", "cooper")
                .attr("password", format!("${{random_password.db_{db_name}.result}}"))
                .attr_ref("db_subnet_group_name", &format!("aws_db_subnet_group.{db_name}.name"))
                .attr("vpc_security_group_ids", json!([format!("${{aws_security_group.app.id}}")]))
                .attr("skip_final_snapshot", true)
                .attr("publicly_accessible", false)
                .attr_map("tags", json!({"Name": format!("{prefix}-{db_name}"), "ManagedBy": "cooper"})),
        ]
    }

    fn map_cache(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            TerraformResource::new("aws_elasticache_subnet_group", "cache")
                .attr("name", format!("{prefix}-cache-subnet-group"))
                .attr("subnet_ids", json!(["${aws_subnet.public_a.id}", "${aws_subnet.public_b.id}"])),

            TerraformResource::new("aws_elasticache_cluster", "cache")
                .attr("cluster_id", format!("{prefix}-cache"))
                .attr("engine", "redis")
                .attr("node_type", "cache.t3.micro")
                .attr("num_cache_nodes", 1)
                .attr("parameter_group_name", "default.redis7")
                .attr_ref("subnet_group_name", "aws_elasticache_subnet_group.cache.name")
                .attr("security_group_ids", json!([format!("${{aws_security_group.app.id}}")])),
        ]
    }

    fn map_topic(&self, topic: &TopicInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        let name = &topic.name;
        vec![
            TerraformResource::new("aws_sns_topic", name)
                .attr("name", format!("{prefix}-{name}"))
                .attr_map("tags", json!({"ManagedBy": "cooper"})),
        ]
    }

    fn map_queue(&self, queue: &QueueInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        let name = &queue.name;
        vec![
            TerraformResource::new("aws_sqs_queue", name)
                .attr("name", format!("{prefix}-{name}"))
                .attr("visibility_timeout_seconds", 30)
                .attr_map("tags", json!({"ManagedBy": "cooper"})),
        ]
    }

    fn map_storage(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            TerraformResource::new("aws_s3_bucket", "storage")
                .attr("bucket", format!("{prefix}-storage"))
                .attr("force_destroy", true)
                .attr_map("tags", json!({"Name": format!("{prefix}-storage"), "ManagedBy": "cooper"})),
        ]
    }

    fn map_iam(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            // ECS Execution Role
            TerraformResource::new("aws_iam_role", "ecs_execution")
                .attr("name", format!("{prefix}-ecs-execution"))
                .attr("assume_role_policy", "${jsonencode({Version=\"2012-10-17\",Statement=[{Action=\"sts:AssumeRole\",Effect=\"Allow\",Principal={Service=\"ecs-tasks.amazonaws.com\"}}]})}"),

            TerraformResource::new("aws_iam_role_policy_attachment", "ecs_execution")
                .attr_ref("role", "aws_iam_role.ecs_execution.name")
                .attr("policy_arn", "arn:aws:iam::aws:policy/service-role/AmazonECSTaskExecutionRolePolicy"),

            // ECS Task Role
            TerraformResource::new("aws_iam_role", "ecs_task")
                .attr("name", format!("{prefix}-ecs-task"))
                .attr("assume_role_policy", "${jsonencode({Version=\"2012-10-17\",Statement=[{Action=\"sts:AssumeRole\",Effect=\"Allow\",Principal={Service=\"ecs-tasks.amazonaws.com\"}}]})}"),
        ]
    }

    fn variables(&self, _config: &MappingConfig) -> Vec<TerraformVariable> {
        vec![
            TerraformVariable::new("aws_region", "string", "AWS region")
                .with_default("us-east-1"),
            TerraformVariable::new("environment", "string", "Environment name (e.g. prod, staging)")
                .with_default(_config.env.as_str()),
            TerraformVariable::new("project_name", "string", "Cooper project name")
                .with_default(_config.project_name.as_str()),
        ]
    }

    fn outputs(&self, _config: &MappingConfig) -> Vec<TerraformOutput> {
        vec![
            TerraformOutput::new(
                "app_url",
                "\"http://${aws_lb.app.dns_name}\"",
                "Public URL to access the application",
            ),
            TerraformOutput::new(
                "ecr_repository_url",
                "aws_ecr_repository.app.repository_url",
                "ECR repository URL for container images",
            ),
            TerraformOutput::new(
                "ecs_cluster_name",
                "aws_ecs_cluster.main.name",
                "ECS cluster name",
            ),
            TerraformOutput::new(
                "ecs_service_name",
                "aws_ecs_service.app.name",
                "ECS service name",
            ),
        ]
    }
}

fn container_definitions_ref(prefix: &str, db_names: &[String]) -> String {
    let mut s = String::from("${jsonencode([{");
    s.push_str("name=\"app\",");
    s.push_str("image=\"${aws_ecr_repository.app.repository_url}:latest\",");
    s.push_str("portMappings=[{containerPort=4000,protocol=\"tcp\"}],");
    s.push_str(&format!(
        "logConfiguration={{logDriver=\"awslogs\",options={{\"awslogs-group\"=\"/ecs/{prefix}\",",
    ));
    s.push_str("\"awslogs-region\"=var.aws_region,\"awslogs-stream-prefix\"=\"ecs\"}},");

    s.push_str("environment=[");
    for (i, db_name) in db_names.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let env_key = format!("COOPER_DB_{}_URL", db_name.to_uppercase());
        let engine = "postgresql";
        s.push_str(&format!(
            "{{name=\"{env_key}\",value=\"{engine}://cooper:${{random_password.db_{db_name}.result}}@${{aws_db_instance.{db_name}.endpoint}}/cooper?sslmode=require\"}}"
        ));
    }
    if !db_names.is_empty() {
        s.push(',');
    }
    s.push_str("{name=\"SECRET\",value=\"${random_password.secret.result}\"}");
    s.push(']');

    s.push_str("}])}");
    s
}
