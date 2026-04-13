use super::{MappingConfig, ResourceMapping};
use super::aws_server::AwsServerMapping;
use crate::terraform::hcl_builder::*;
use cooper_codegen::analyzer::*;
use serde_json::json;

/// AWS Serverless mapping: Lambda + API Gateway instead of ECS Fargate.
/// Database, cache, messaging, and storage mappings are the same as the server mode.
pub struct AwsLambdaMapping;

impl ResourceMapping for AwsLambdaMapping {
    fn providers(&self, config: &MappingConfig) -> Vec<TerraformProvider> {
        // Same providers as server
        AwsServerMapping.providers(config)
    }

    fn map_networking(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        // Same networking as server
        AwsServerMapping.map_networking(config)
    }

    fn map_compute(&self, _routes: &[RouteInfo], config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            // Lambda Function
            TerraformResource::new("aws_lambda_function", "api")
                .attr("function_name", format!("{prefix}-api"))
                .attr("runtime", "nodejs20.x")
                .attr("handler", "index.handler")
                .attr_ref("role", "aws_iam_role.lambda_exec.arn")
                .attr("filename", "lambda.zip")
                .attr("source_code_hash", "${filebase64sha256(\"lambda.zip\")}")
                .attr("timeout", 30)
                .attr("memory_size", 512)
                .attr_block("vpc_config", json!({
                    "subnet_ids": ["${aws_subnet.public_a.id}", "${aws_subnet.public_b.id}"],
                    "security_group_ids": ["${aws_security_group.app.id}"]
                }))
                .attr_block("environment", json!({
                    "variables": {
                        "NODE_ENV": "production",
                        "COOPER_ENV": config.env
                    }
                }))
                .attr_block("tags", json!({"ManagedBy": "cooper"})),

            // API Gateway HTTP API
            TerraformResource::new("aws_apigatewayv2_api", "api")
                .attr("name", format!("{prefix}-api"))
                .attr("protocol_type", "HTTP"),

            // API Gateway Integration
            TerraformResource::new("aws_apigatewayv2_integration", "lambda")
                .attr_ref("api_id", "aws_apigatewayv2_api.api.id")
                .attr("integration_type", "AWS_PROXY")
                .attr_ref("integration_uri", "aws_lambda_function.api.invoke_arn")
                .attr("payload_format_version", "2.0"),

            // Default Route
            TerraformResource::new("aws_apigatewayv2_route", "default")
                .attr_ref("api_id", "aws_apigatewayv2_api.api.id")
                .attr("route_key", "$default")
                .attr("target", "integrations/${aws_apigatewayv2_integration.lambda.id}"),

            // Auto-deploy Stage
            TerraformResource::new("aws_apigatewayv2_stage", "default")
                .attr_ref("api_id", "aws_apigatewayv2_api.api.id")
                .attr("name", "$default")
                .attr("auto_deploy", true),

            // Lambda Permission for API Gateway
            TerraformResource::new("aws_lambda_permission", "api_gw")
                .attr("statement_id", "AllowAPIGatewayInvoke")
                .attr("action", "lambda:InvokeFunction")
                .attr_ref("function_name", "aws_lambda_function.api.function_name")
                .attr("principal", "apigateway.amazonaws.com")
                .attr("source_arn", "${aws_apigatewayv2_api.api.execution_arn}/*/*"),

            // CloudWatch Log Group for Lambda
            TerraformResource::new("aws_cloudwatch_log_group", "lambda")
                .attr("name", format!("/aws/lambda/{prefix}-api"))
                .attr("retention_in_days", 14),
        ]
    }

    fn map_database(&self, db: &DatabaseInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        AwsServerMapping.map_database(db, config)
    }

    fn map_cache(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        AwsServerMapping.map_cache(config)
    }

    fn map_topic(&self, topic: &TopicInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        AwsServerMapping.map_topic(topic, config)
    }

    fn map_queue(&self, queue: &QueueInfo, config: &MappingConfig) -> Vec<TerraformResource> {
        AwsServerMapping.map_queue(queue, config)
    }

    fn map_storage(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        AwsServerMapping.map_storage(config)
    }

    fn map_iam(&self, config: &MappingConfig) -> Vec<TerraformResource> {
        let prefix = &config.prefix;
        vec![
            // Lambda Execution Role
            TerraformResource::new("aws_iam_role", "lambda_exec")
                .attr("name", format!("{prefix}-lambda-exec"))
                .attr("assume_role_policy", "${jsonencode({Version=\"2012-10-17\",Statement=[{Action=\"sts:AssumeRole\",Effect=\"Allow\",Principal={Service=\"lambda.amazonaws.com\"}}]})}"),

            // Basic Lambda execution policy
            TerraformResource::new("aws_iam_role_policy_attachment", "lambda_basic")
                .attr_ref("role", "aws_iam_role.lambda_exec.name")
                .attr("policy_arn", "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"),

            // VPC access policy
            TerraformResource::new("aws_iam_role_policy_attachment", "lambda_vpc")
                .attr_ref("role", "aws_iam_role.lambda_exec.name")
                .attr("policy_arn", "arn:aws:iam::aws:policy/service-role/AWSLambdaVPCAccessExecutionRole"),
        ]
    }

    fn variables(&self, config: &MappingConfig) -> Vec<TerraformVariable> {
        AwsServerMapping.variables(config)
    }

    fn outputs(&self, _config: &MappingConfig) -> Vec<TerraformOutput> {
        vec![
            TerraformOutput::new(
                "api_gateway_url",
                "aws_apigatewayv2_stage.default.invoke_url",
                "API Gateway URL",
            ),
            TerraformOutput::new(
                "lambda_function_name",
                "aws_lambda_function.api.function_name",
                "Lambda function name",
            ),
        ]
    }
}
