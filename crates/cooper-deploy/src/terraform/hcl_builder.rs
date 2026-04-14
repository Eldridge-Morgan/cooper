use anyhow::Result;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A single Terraform resource block.
#[derive(Debug, Clone)]
pub struct TerraformResource {
    pub type_name: String,
    pub name: String,
    pub attributes: BTreeMap<String, Value>,
    /// Keys that are map arguments (rendered with `=`) rather than nested blocks.
    map_keys: BTreeSet<String>,
}

impl TerraformResource {
    pub fn new(type_name: &str, name: &str) -> Self {
        Self {
            type_name: type_name.to_string(),
            name: name.to_string(),
            attributes: BTreeMap::new(),
            map_keys: BTreeSet::new(),
        }
    }

    pub fn attr(mut self, key: &str, value: impl Into<Value>) -> Self {
        self.attributes.insert(key.to_string(), value.into());
        self
    }

    pub fn attr_ref(mut self, key: &str, reference: &str) -> Self {
        self.attributes
            .insert(key.to_string(), Value::String(format!("${{{reference}}}")));
        self
    }

    pub fn attr_block(mut self, key: &str, block: Value) -> Self {
        self.attributes.insert(key.to_string(), block);
        self
    }

    /// Add a map-type argument (rendered as `key = { ... }` with `=`).
    /// Use this for `tags`, `app_settings`, etc. — NOT for nested blocks.
    pub fn attr_map(mut self, key: &str, map: Value) -> Self {
        self.map_keys.insert(key.to_string());
        self.attributes.insert(key.to_string(), map);
        self
    }

    pub fn to_hcl(&self) -> String {
        let mut hcl = format!(
            "resource \"{}\" \"{}\" {{\n",
            self.type_name, self.name
        );
        for (key, value) in &self.attributes {
            if self.map_keys.contains(key) {
                // Map argument: key = { ... }
                hcl.push_str(&format!("  {key} = {}\n", value_to_hcl(value)));
            } else {
                write_attribute(&mut hcl, key, value, 2);
            }
        }
        hcl.push_str("}\n");
        hcl
    }
}

/// A Terraform variable declaration.
#[derive(Debug, Clone)]
pub struct TerraformVariable {
    pub name: String,
    pub var_type: String,
    pub description: String,
    pub default: Option<Value>,
    pub sensitive: bool,
}

impl TerraformVariable {
    pub fn new(name: &str, var_type: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            var_type: var_type.to_string(),
            description: description.to_string(),
            default: None,
            sensitive: false,
        }
    }

    pub fn with_default(mut self, default: impl Into<Value>) -> Self {
        self.default = Some(default.into());
        self
    }

    pub fn sensitive(mut self) -> Self {
        self.sensitive = true;
        self
    }

    pub fn to_hcl(&self) -> String {
        let mut hcl = format!("variable \"{}\" {{\n", self.name);
        hcl.push_str(&format!("  type        = {}\n", self.var_type));
        hcl.push_str(&format!(
            "  description = \"{}\"\n",
            self.description.replace('"', "\\\"")
        ));
        if let Some(default) = &self.default {
            hcl.push_str(&format!("  default     = {}\n", value_to_hcl(default)));
        }
        if self.sensitive {
            hcl.push_str("  sensitive   = true\n");
        }
        hcl.push_str("}\n");
        hcl
    }
}

/// A Terraform output declaration.
#[derive(Debug, Clone)]
pub struct TerraformOutput {
    pub name: String,
    pub value: String,
    pub description: String,
    pub sensitive: bool,
}

impl TerraformOutput {
    pub fn new(name: &str, value: &str, description: &str) -> Self {
        Self {
            name: name.to_string(),
            value: value.to_string(),
            description: description.to_string(),
            sensitive: false,
        }
    }

    pub fn sensitive(mut self) -> Self {
        self.sensitive = true;
        self
    }

    pub fn to_hcl(&self) -> String {
        let mut hcl = format!("output \"{}\" {{\n", self.name);
        hcl.push_str(&format!("  value       = {}\n", self.value));
        hcl.push_str(&format!(
            "  description = \"{}\"\n",
            self.description.replace('"', "\\\"")
        ));
        if self.sensitive {
            hcl.push_str("  sensitive   = true\n");
        }
        hcl.push_str("}\n");
        hcl
    }
}

/// A Terraform provider block.
#[derive(Debug, Clone)]
pub struct TerraformProvider {
    pub name: String,
    pub source: String,
    pub version: String,
    pub config: BTreeMap<String, Value>,
}

/// Complete Terraform configuration that can be written to disk.
#[derive(Debug, Clone)]
pub struct TerraformConfig {
    pub providers: Vec<TerraformProvider>,
    pub resources: Vec<TerraformResource>,
    pub variables: Vec<TerraformVariable>,
    pub outputs: Vec<TerraformOutput>,
    pub data_sources: Vec<TerraformResource>,
    /// Extra HCL blocks (e.g., `locals`, `data` blocks) as raw HCL strings.
    pub extra_blocks: Vec<String>,
}

impl TerraformConfig {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            resources: Vec::new(),
            variables: Vec::new(),
            outputs: Vec::new(),
            data_sources: Vec::new(),
            extra_blocks: Vec::new(),
        }
    }

    /// Write all Terraform files to the given directory.
    pub fn write_to_disk(&self, dir: &str) -> Result<()> {
        std::fs::create_dir_all(dir)?;

        // provider.tf
        std::fs::write(format!("{dir}/provider.tf"), self.provider_hcl())?;

        // main.tf
        std::fs::write(format!("{dir}/main.tf"), self.main_hcl())?;

        // variables.tf
        std::fs::write(format!("{dir}/variables.tf"), self.variables_hcl())?;

        // outputs.tf
        std::fs::write(format!("{dir}/outputs.tf"), self.outputs_hcl())?;

        Ok(())
    }

    fn provider_hcl(&self) -> String {
        let mut hcl = String::new();

        // terraform block with required_providers
        hcl.push_str("terraform {\n  required_version = \">= 1.0\"\n\n  required_providers {\n");
        for provider in &self.providers {
            hcl.push_str(&format!(
                "    {} = {{\n      source  = \"{}\"\n      version = \"~> {}\"\n    }}\n",
                provider.name, provider.source, provider.version
            ));
        }
        hcl.push_str("  }\n}\n\n");

        // Provider configuration blocks
        for provider in &self.providers {
            hcl.push_str(&format!("provider \"{}\" {{\n", provider.name));
            for (key, value) in &provider.config {
                write_attribute(&mut hcl, key, value, 2);
            }
            hcl.push_str("}\n\n");
        }

        hcl
    }

    fn main_hcl(&self) -> String {
        let mut hcl = String::from(
            "# Generated by Cooper — do not edit directly.\n# Regenerate with: cooper deploy --env <env> --cloud <cloud>\n\n",
        );

        // Extra blocks (locals, data sources, etc.)
        for block in &self.extra_blocks {
            hcl.push_str(block);
            hcl.push_str("\n\n");
        }

        // Data sources
        for ds in &self.data_sources {
            hcl.push_str(&format!(
                "data \"{}\" \"{}\" {{\n",
                ds.type_name, ds.name
            ));
            for (key, value) in &ds.attributes {
                write_attribute(&mut hcl, key, value, 2);
            }
            hcl.push_str("}\n\n");
        }

        // Resources
        for resource in &self.resources {
            hcl.push_str(&resource.to_hcl());
            hcl.push('\n');
        }

        hcl
    }

    fn variables_hcl(&self) -> String {
        let mut hcl = String::from(
            "# Generated by Cooper — do not edit directly.\n\n",
        );
        for variable in &self.variables {
            hcl.push_str(&variable.to_hcl());
            hcl.push('\n');
        }
        hcl
    }

    fn outputs_hcl(&self) -> String {
        let mut hcl = String::from(
            "# Generated by Cooper — do not edit directly.\n\n",
        );
        for output in &self.outputs {
            hcl.push_str(&output.to_hcl());
            hcl.push('\n');
        }
        hcl
    }

    /// Format a preview summary of resources for display.
    pub fn format_preview(&self) -> String {
        let mut preview = String::new();
        preview.push_str("  Resources to be created:\n");
        preview.push_str("  ─────────────────────────────────────────\n");

        for resource in &self.resources {
            let cost = estimate_cost(&resource.type_name);
            let cost_str = if cost > 0.0 {
                format!("  ~${:.0}/mo", cost)
            } else {
                String::new()
            };
            preview.push_str(&format!(
                "  + {}.{}{}\n",
                resource.type_name, resource.name, cost_str
            ));
        }

        preview.push_str("  ─────────────────────────────────────────\n");

        let total: f64 = self
            .resources
            .iter()
            .map(|r| estimate_cost(&r.type_name))
            .sum();
        if total > 0.0 {
            preview.push_str(&format!("  Estimated monthly cost: ~${:.0}/mo\n", total));
        }

        preview
    }
}

/// Convert a serde_json::Value to HCL representation.
pub fn value_to_hcl(value: &Value) -> String {
    match value {
        Value::String(s) => {
            if s.starts_with("var.") || s.starts_with("local.") || s.starts_with("module.") {
                // Already a bare reference
                s.clone()
            } else if s.starts_with("${") {
                if s.ends_with('}') {
                    // Pure reference or function call wrapped in ${...} — strip the outer wrapper.
                    // e.g. ${aws_vpc.main.id}       → aws_vpc.main.id
                    //      ${jsonencode({...})}      → jsonencode({...})
                    //      ${"${var.aws_region}a"}   → "${var.aws_region}a"  (valid quoted HCL)
                    s[2..s.len() - 1].to_string()
                } else {
                    // Reference embedded in a larger string, e.g. ${ref}/*/*
                    // Wrap in quotes so it becomes a valid HCL string interpolation.
                    format!("\"{}\"", s)
                }
            } else {
                format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
            }
        }
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(value_to_hcl).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            let mut hcl = String::from("{\n");
            for (k, v) in obj {
                hcl.push_str(&format!("    {} = {}\n", k, value_to_hcl(v)));
            }
            hcl.push_str("  }");
            hcl
        }
        Value::Null => "null".to_string(),
    }
}

/// Keys that are map arguments (need `= { ... }`) rather than nested blocks.
fn is_map_attribute(key: &str) -> bool {
    matches!(key, "tags" | "variables" | "app_settings" | "labels")
}

/// Write a single key = value attribute to HCL, handling nested blocks.
fn write_attribute(hcl: &mut String, key: &str, value: &Value, indent: usize) {
    let pad = " ".repeat(indent);
    match value {
        Value::Object(obj) if !key.is_empty() && !is_map_attribute(key) => {
            // Nested block (no `=`)
            hcl.push_str(&format!("{pad}{key} {{\n"));
            for (k, v) in obj {
                write_attribute(hcl, k, v, indent + 2);
            }
            hcl.push_str(&format!("{pad}}}\n"));
        }
        _ => {
            hcl.push_str(&format!("{pad}{key} = {}\n", value_to_hcl(value)));
        }
    }
}

fn estimate_cost(resource_type: &str) -> f64 {
    match resource_type {
        // AWS
        "aws_db_instance" => 28.0,
        "aws_elasticache_cluster" => 12.0,
        "aws_ecs_service" => 0.0,
        "aws_lambda_function" => 0.0,
        // GCP
        "google_sql_database_instance" => 10.0,
        "google_redis_instance" => 10.0,
        "google_cloud_run_v2_service" => 0.0,
        "google_cloudfunctions2_function" => 0.0,
        // Azure
        "azurerm_postgresql_flexible_server" => 15.0,
        "azurerm_redis_cache" => 13.0,
        "azurerm_container_app" => 0.0,
        "azurerm_linux_function_app" => 0.0,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_to_hcl() {
        let r = TerraformResource::new("aws_ecs_cluster", "main")
            .attr("name", "my-cluster")
            .attr_ref("vpc_id", "aws_vpc.main.id");

        let hcl = r.to_hcl();
        assert!(hcl.contains("resource \"aws_ecs_cluster\" \"main\""));
        assert!(hcl.contains("name = \"my-cluster\""));
        assert!(hcl.contains("vpc_id = ${aws_vpc.main.id}"));
    }

    #[test]
    fn test_variable_to_hcl() {
        let v = TerraformVariable::new("region", "string", "AWS region")
            .with_default("us-east-1");
        let hcl = v.to_hcl();
        assert!(hcl.contains("variable \"region\""));
        assert!(hcl.contains("default     = \"us-east-1\""));
    }

    #[test]
    fn test_output_to_hcl() {
        let o = TerraformOutput::new("app_url", "aws_lb.main.dns_name", "Application URL");
        let hcl = o.to_hcl();
        assert!(hcl.contains("output \"app_url\""));
        assert!(hcl.contains("value       = aws_lb.main.dns_name"));
    }

    #[test]
    fn test_value_to_hcl_reference() {
        assert_eq!(
            value_to_hcl(&Value::String("${aws_vpc.main.id}".to_string())),
            "${aws_vpc.main.id}"
        );
    }

    #[test]
    fn test_value_to_hcl_string() {
        assert_eq!(
            value_to_hcl(&Value::String("hello".to_string())),
            "\"hello\""
        );
    }
}
