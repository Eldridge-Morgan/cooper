use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAnalysis {
    pub routes: Vec<RouteInfo>,
    pub topics: Vec<TopicInfo>,
    pub databases: Vec<DatabaseInfo>,
    pub crons: Vec<CronInfo>,
    pub queues: Vec<QueueInfo>,
    pub pages: Vec<PageInfo>,
}

impl ProjectAnalysis {
    pub fn has_databases(&self) -> bool {
        !self.databases.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub method: String,
    pub path: String,
    pub auth: bool,
    pub stream: Option<StreamKind>,
    pub has_validation: bool,
    pub export_name: String,
    pub source_file: String,
    pub middleware: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamKind {
    Sse,
    WebSocket,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicInfo {
    pub name: String,
    pub export_name: String,
    pub source_file: String,
    pub delivery_guarantee: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub engine: String,
    pub migrations: Option<String>,
    pub export_name: String,
    pub source_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronInfo {
    pub name: String,
    pub schedule: String,
    pub export_name: String,
    pub source_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueInfo {
    pub name: String,
    pub export_name: String,
    pub source_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    pub route: String,
    pub source_file: String,
    pub has_layout: bool,
}

/// Analyze a Cooper project by scanning TypeScript files for declarations.
pub fn analyze(project_root: &Path) -> Result<ProjectAnalysis> {
    let mut analysis = ProjectAnalysis {
        routes: Vec::new(),
        topics: Vec::new(),
        databases: Vec::new(),
        crons: Vec::new(),
        queues: Vec::new(),
        pages: Vec::new(),
    };

    // Scan services/ for API declarations
    let services_dir = project_root.join("services");
    if services_dir.exists() {
        scan_directory(&services_dir, project_root, &mut analysis)?;
    }

    // Scan pages/ for SSR routes
    let pages_dir = project_root.join("pages");
    if pages_dir.exists() {
        scan_pages(&pages_dir, project_root, &mut analysis)?;
    }

    // Scan root .ts files
    for entry in std::fs::read_dir(project_root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_ts_file(&path) {
            let rel = path
                .strip_prefix(project_root)?
                .to_string_lossy()
                .to_string();
            analyze_file(&path, &rel, &mut analysis)?;
        }
    }

    Ok(analysis)
}

fn scan_directory(dir: &Path, root: &Path, analysis: &mut ProjectAnalysis) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_directory(&path, root, analysis)?;
        } else if is_ts_file(&path) {
            let rel = path.strip_prefix(root)?.to_string_lossy().to_string();
            analyze_file(&path, &rel, analysis)?;
        }
    }
    Ok(())
}

fn scan_pages(dir: &Path, root: &Path, analysis: &mut ProjectAnalysis) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            scan_pages(&path, root, analysis)?;
        } else if is_ts_file(&path) {
            let rel = path.strip_prefix(root)?.to_string_lossy().to_string();
            let route = file_path_to_route(&rel);
            let is_layout = path
                .file_stem()
                .map(|s| s.to_string_lossy().starts_with('_'))
                .unwrap_or(false);

            analysis.pages.push(PageInfo {
                route,
                source_file: rel,
                has_layout: is_layout,
            });
        }
    }
    Ok(())
}

fn is_ts_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("ts" | "tsx")
    )
}

/// Convert "pages/users/[id].tsx" → "/users/:id"
fn file_path_to_route(rel_path: &str) -> String {
    let path = rel_path
        .strip_prefix("pages")
        .unwrap_or(rel_path)
        .trim_start_matches('/');

    let path = path
        .strip_suffix(".tsx")
        .or_else(|| path.strip_suffix(".ts"))
        .unwrap_or(path);

    let segments: Vec<String> = path
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| {
            if s == "index" {
                String::new()
            } else if s.starts_with('[') && s.ends_with(']') {
                let param = &s[1..s.len() - 1];
                if let Some(rest) = param.strip_prefix("...") {
                    format!("*{rest}")
                } else {
                    format!(":{param}")
                }
            } else {
                s.to_string()
            }
        })
        .collect();

    let route = format!("/{}", segments.join("/"));
    if route.ends_with('/') && route.len() > 1 {
        route.trim_end_matches('/').to_string()
    } else {
        route
    }
}

/// Parse a TypeScript file using regex to extract Cooper declarations.
///
/// We look for patterns like:
///   export const NAME = api({ method: "GET", path: "/foo", ... }, ...)
///   export const NAME = topic<...>("name", ...)
///   export const NAME = database("name", { engine: "postgres", ... })
///   export const NAME = cron("name", { schedule: "...", ... })
///   export const NAME = queue<...>("name", ...)
fn analyze_file(path: &Path, rel_path: &str, analysis: &mut ProjectAnalysis) -> Result<()> {
    let source = std::fs::read_to_string(path)?;

    extract_api_routes(&source, rel_path, analysis);
    extract_topics(&source, rel_path, analysis);
    extract_databases(&source, rel_path, analysis);
    extract_crons(&source, rel_path, analysis);
    extract_queues(&source, rel_path, analysis);

    Ok(())
}

fn extract_api_routes(source: &str, source_file: &str, analysis: &mut ProjectAnalysis) {
    // Match: export const NAME = api(
    //   { method: "METHOD", path: "/path", auth: true/false, stream: "sse"/"websocket", validate: ... },
    let re = Regex::new(
        r#"export\s+const\s+(\w+)\s*=\s*api\s*\(\s*\{([^}]*)\}"#
    ).unwrap();

    for cap in re.captures_iter(source) {
        let export_name = cap[1].to_string();
        let config_body = &cap[2];

        let method = extract_string_prop(config_body, "method")
            .unwrap_or_else(|| "GET".to_string());
        let path = match extract_string_prop(config_body, "path") {
            Some(p) => p,
            None => continue,
        };
        let auth = extract_bool_prop(config_body, "auth").unwrap_or(false);
        let stream = extract_string_prop(config_body, "stream").and_then(|s| match s.as_str() {
            "sse" => Some(StreamKind::Sse),
            "websocket" => Some(StreamKind::WebSocket),
            _ => None,
        });
        let has_validation = config_body.contains("validate:");

        analysis.routes.push(RouteInfo {
            method,
            path,
            auth,
            stream,
            has_validation,
            export_name,
            source_file: source_file.to_string(),
            middleware: Vec::new(),
        });
    }
}

fn extract_topics(source: &str, source_file: &str, analysis: &mut ProjectAnalysis) {
    let re = Regex::new(
        r#"export\s+const\s+(\w+)\s*=\s*topic(?:<[^>]*>)?\s*\(\s*"([^"]+)""#
    ).unwrap();

    for cap in re.captures_iter(source) {
        analysis.topics.push(TopicInfo {
            name: cap[2].to_string(),
            export_name: cap[1].to_string(),
            source_file: source_file.to_string(),
            delivery_guarantee: None,
        });
    }
}

fn extract_databases(source: &str, source_file: &str, analysis: &mut ProjectAnalysis) {
    let re = Regex::new(
        r#"export\s+const\s+(\w+)\s*=\s*database\s*\(\s*"([^"]+)"(?:\s*,\s*\{([^}]*)\})?"#
    ).unwrap();

    for cap in re.captures_iter(source) {
        let config_body = cap.get(3).map(|m| m.as_str()).unwrap_or("");
        let engine = extract_string_prop(config_body, "engine")
            .unwrap_or_else(|| "postgres".to_string());
        let migrations = extract_string_prop(config_body, "migrations");

        analysis.databases.push(DatabaseInfo {
            name: cap[2].to_string(),
            engine,
            migrations,
            export_name: cap[1].to_string(),
            source_file: source_file.to_string(),
        });
    }
}

fn extract_crons(source: &str, source_file: &str, analysis: &mut ProjectAnalysis) {
    let re = Regex::new(
        r#"export\s+const\s+(\w+)\s*=\s*cron\s*\(\s*"([^"]+)"\s*,\s*\{([^}]*)\}"#
    ).unwrap();

    for cap in re.captures_iter(source) {
        let config_body = &cap[3];
        let schedule = extract_string_prop(config_body, "schedule")
            .unwrap_or_default();

        analysis.crons.push(CronInfo {
            name: cap[2].to_string(),
            schedule,
            export_name: cap[1].to_string(),
            source_file: source_file.to_string(),
        });
    }
}

fn extract_queues(source: &str, source_file: &str, analysis: &mut ProjectAnalysis) {
    let re = Regex::new(
        r#"export\s+const\s+(\w+)\s*=\s*queue(?:<[^>]*>)?\s*\(\s*"([^"]+)""#
    ).unwrap();

    for cap in re.captures_iter(source) {
        analysis.queues.push(QueueInfo {
            name: cap[2].to_string(),
            export_name: cap[1].to_string(),
            source_file: source_file.to_string(),
        });
    }
}

/// Extract a string property value from a JS object literal body.
/// e.g. extract_string_prop("method: \"GET\", path: \"/foo\"", "method") → Some("GET")
fn extract_string_prop(body: &str, key: &str) -> Option<String> {
    let re = Regex::new(&format!(r#"{}:\s*"([^"]*)""#, regex::escape(key))).ok()?;
    re.captures(body).map(|c| c[1].to_string())
}

/// Extract a boolean property value.
fn extract_bool_prop(body: &str, key: &str) -> Option<bool> {
    let re = Regex::new(&format!(r#"{}:\s*(true|false)"#, regex::escape(key))).ok()?;
    re.captures(body).map(|c| &c[1] == "true")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_path_to_route() {
        assert_eq!(file_path_to_route("pages/index.tsx"), "/");
        assert_eq!(file_path_to_route("pages/about.tsx"), "/about");
        assert_eq!(file_path_to_route("pages/users/[id].tsx"), "/users/:id");
        assert_eq!(
            file_path_to_route("pages/blog/[...slug].tsx"),
            "/blog/*slug"
        );
    }

    #[test]
    fn test_extract_api_routes() {
        let source = r#"
export const getUser = api(
  { method: "GET", path: "/users/:id", auth: true },
  async ({ id }) => { return { id }; }
);

export const createUser = api(
  { method: "POST", path: "/users", validate: CreateUserSchema },
  async (req) => { return req; }
);
"#;
        let mut analysis = ProjectAnalysis {
            routes: vec![],
            topics: vec![],
            databases: vec![],
            crons: vec![],
            queues: vec![],
            pages: vec![],
        };

        extract_api_routes(source, "services/users/api.ts", &mut analysis);

        assert_eq!(analysis.routes.len(), 2);
        assert_eq!(analysis.routes[0].method, "GET");
        assert_eq!(analysis.routes[0].path, "/users/:id");
        assert!(analysis.routes[0].auth);
        assert_eq!(analysis.routes[0].export_name, "getUser");

        assert_eq!(analysis.routes[1].method, "POST");
        assert_eq!(analysis.routes[1].path, "/users");
        assert!(analysis.routes[1].has_validation);
    }

    #[test]
    fn test_extract_databases() {
        let source = r#"
export const db = database("main", {
  engine: "postgres",
  migrations: "./migrations",
});

export const mongo = database("catalog", { engine: "mongodb" });
"#;
        let mut analysis = ProjectAnalysis {
            routes: vec![],
            topics: vec![],
            databases: vec![],
            crons: vec![],
            queues: vec![],
            pages: vec![],
        };

        extract_databases(source, "services/users/api.ts", &mut analysis);

        assert_eq!(analysis.databases.len(), 2);
        assert_eq!(analysis.databases[0].name, "main");
        assert_eq!(analysis.databases[0].engine, "postgres");
        assert_eq!(
            analysis.databases[0].migrations,
            Some("./migrations".to_string())
        );
        assert_eq!(analysis.databases[1].name, "catalog");
        assert_eq!(analysis.databases[1].engine, "mongodb");
    }

    #[test]
    fn test_extract_topics() {
        let source = r#"
export const UserSignedUp = topic<{ userId: string }>("user-signed-up", {
  deliveryGuarantee: "at-least-once",
});
"#;
        let mut analysis = ProjectAnalysis {
            routes: vec![],
            topics: vec![],
            databases: vec![],
            crons: vec![],
            queues: vec![],
            pages: vec![],
        };

        extract_topics(source, "events.ts", &mut analysis);

        assert_eq!(analysis.topics.len(), 1);
        assert_eq!(analysis.topics[0].name, "user-signed-up");
        assert_eq!(analysis.topics[0].export_name, "UserSignedUp");
    }
}
