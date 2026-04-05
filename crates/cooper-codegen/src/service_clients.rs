use crate::analyzer::{ProjectAnalysis, RouteInfo};
use std::collections::HashMap;

/// Generate service-to-service TypeScript clients.
/// Groups routes by source file directory (service) and generates
/// a typed client for each service under `~gen/clients/`.
pub fn generate_service_clients(analysis: &ProjectAnalysis) -> HashMap<String, String> {
    let mut services: HashMap<String, Vec<&RouteInfo>> = HashMap::new();

    for route in &analysis.routes {
        // Extract service name from source path: "services/users/api.ts" → "users"
        let parts: Vec<&str> = route.source_file.split('/').collect();
        let service_name = if parts.len() >= 2 && parts[0] == "services" {
            parts[1].to_string()
        } else {
            "default".to_string()
        };

        services.entry(service_name).or_default().push(route);
    }

    let mut result = HashMap::new();

    for (service, routes) in &services {
        let class_name = format!("{}Service", capitalize(service));
        let mut code = format!(
            r#"// Auto-generated service client for "{service}"
// Import: import {{ {class_name} }} from "~gen/clients/{service}";

const BASE = process.env.COOPER_SERVICE_{upper}_URL ?? "http://localhost:4000";

async function request(method: string, path: string, opts?: {{ body?: any; params?: Record<string, string>; headers?: Record<string, string> }}): Promise<any> {{
  let url = BASE + path;
  if (opts?.params) {{
    for (const [k, v] of Object.entries(opts.params)) {{
      url = url.replace(`:${{k}}`, encodeURIComponent(v));
    }}
  }}
  const res = await fetch(url, {{
    method,
    headers: {{ "Content-Type": "application/json", ...opts?.headers }},
    body: opts?.body ? JSON.stringify(opts.body) : undefined,
  }});
  if (!res.ok) throw await res.json();
  return res.json();
}}

export const {class_name} = {{
"#,
            upper = service.to_uppercase()
        );

        for route in routes {
            let method = &route.method;
            let path = &route.path;
            let name = &route.export_name;

            let params: Vec<&str> = path.split('/').filter_map(|s| s.strip_prefix(':')).collect();
            let has_body = matches!(method.as_str(), "POST" | "PUT" | "PATCH");

            let mut args = Vec::new();
            for p in &params {
                args.push(format!("{p}: string"));
            }
            if has_body {
                args.push("body: any".to_string());
            }

            let mut opts_parts = Vec::new();
            if !params.is_empty() {
                let map: Vec<String> = params.iter().map(|p| p.to_string()).collect();
                opts_parts.push(format!("params: {{ {} }}", map.join(", ")));
            }
            if has_body {
                opts_parts.push("body".to_string());
            }

            let opts = if opts_parts.is_empty() {
                String::new()
            } else {
                format!(", {{ {} }}", opts_parts.join(", "))
            };

            code.push_str(&format!(
                "  async {name}({args}): Promise<any> {{\n    return request(\"{method}\", \"{path}\"{opts});\n  }},\n\n",
                args = args.join(", ")
            ));
        }

        code.push_str("};\n");
        result.insert(service.clone(), code);
    }

    // Generate index that re-exports all services
    let mut index = String::new();
    for service in services.keys() {
        let class_name = format!("{}Service", capitalize(service));
        index.push_str(&format!(
            "export {{ {class_name} }} from \"./{service}\";\n"
        ));
    }
    result.insert("index".to_string(), index);

    result
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
