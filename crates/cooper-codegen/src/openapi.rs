use crate::analyzer::{ProjectAnalysis, RouteInfo};
use serde_json::{json, Value};

/// Generate an OpenAPI 3.1 spec from the project analysis
pub fn generate(analysis: &ProjectAnalysis, title: &str, version: &str) -> Value {
    let mut paths = serde_json::Map::new();

    for route in &analysis.routes {
        let path_entry = paths
            .entry(route.path.clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));

        let method = route.method.to_lowercase();
        let operation = build_operation(route);

        if let Value::Object(map) = path_entry {
            map.insert(method, operation);
        }
    }

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": title,
            "version": version,
        },
        "paths": paths,
    })
}

fn build_operation(route: &RouteInfo) -> Value {
    let mut operation = json!({
        "operationId": route.export_name,
        "responses": {
            "200": {
                "description": "Success",
                "content": {
                    "application/json": {
                        "schema": { "type": "object" }
                    }
                }
            }
        }
    });

    if route.auth {
        operation["security"] = json!([{ "bearerAuth": [] }]);
    }

    // Extract path parameters
    let params: Vec<Value> = route
        .path
        .split('/')
        .filter_map(|seg| {
            seg.strip_prefix(':').map(|name| {
                json!({
                    "name": name,
                    "in": "path",
                    "required": true,
                    "schema": { "type": "string" }
                })
            })
        })
        .collect();

    if !params.is_empty() {
        operation["parameters"] = Value::Array(params);
    }

    if matches!(route.method.as_str(), "POST" | "PUT" | "PATCH") {
        operation["requestBody"] = json!({
            "content": {
                "application/json": {
                    "schema": { "type": "object" }
                }
            }
        });
    }

    operation
}
