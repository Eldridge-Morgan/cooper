use anyhow::Result;
use regex::Regex;
use std::path::{Path, PathBuf};

/// A Cooper workspace — multiple apps in a monorepo.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub apps: Vec<WorkspaceApp>,
    pub shared: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceApp {
    pub name: String,
    pub path: PathBuf,
    pub analysis: crate::analyzer::ProjectAnalysis,
}

/// Detect and parse a Cooper workspace.
///
/// A workspace is identified by `cooper.workspace.ts` at the root.
/// Falls back to checking for multiple `cooper.config.ts` files in subdirs.
pub fn detect_workspace(root: &Path) -> Result<Option<Workspace>> {
    let ws_config = root.join("cooper.workspace.ts");
    if ws_config.exists() {
        return parse_workspace_config(root, &ws_config);
    }

    // Also check for JS variant
    let ws_config_js = root.join("cooper.workspace.js");
    if ws_config_js.exists() {
        return parse_workspace_config(root, &ws_config_js);
    }

    // No workspace file — check if this looks like a monorepo by scanning
    // for cooper.config.ts in immediate subdirectories
    let mut apps = Vec::new();
    let candidates = ["apps", "packages", "services"];

    for dir_name in &candidates {
        let dir = root.join(dir_name);
        if !dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && path.join("cooper.config.ts").exists() {
                apps.push(path);
            }
        }
    }

    if apps.is_empty() {
        return Ok(None);
    }

    // Found apps — build workspace
    let mut workspace_apps = Vec::new();
    for app_path in &apps {
        let name = app_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let analysis = crate::analyzer::analyze(app_path)?;
        workspace_apps.push(WorkspaceApp {
            name,
            path: app_path.clone(),
            analysis,
        });
    }

    // Collect shared packages
    let mut shared = Vec::new();
    let packages_dir = root.join("packages");
    if packages_dir.exists() {
        for entry in std::fs::read_dir(&packages_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() && !path.join("cooper.config.ts").exists() {
                shared.push(path);
            }
        }
    }

    Ok(Some(Workspace {
        root: root.to_path_buf(),
        apps: workspace_apps,
        shared,
    }))
}

/// Parse cooper.workspace.ts to extract app and shared paths.
///
/// Looks for patterns like:
///   apps: ["apps/api", "apps/workers"]
///   shared: ["packages/*"]
fn parse_workspace_config(root: &Path, config_path: &Path) -> Result<Option<Workspace>> {
    let content = std::fs::read_to_string(config_path)?;

    // Extract apps array
    let apps_re = Regex::new(r#"apps:\s*\[([^\]]*)\]"#)?;
    let shared_re = Regex::new(r#"shared:\s*\[([^\]]*)\]"#)?;
    let string_re = Regex::new(r#""([^"]+)""#)?;

    let mut app_paths = Vec::new();
    if let Some(cap) = apps_re.captures(&content) {
        let list = &cap[1];
        for s in string_re.captures_iter(list) {
            let p = &s[1];
            let full = root.join(p);
            if full.exists() {
                app_paths.push(full);
            }
        }
    }

    let mut shared_paths = Vec::new();
    if let Some(cap) = shared_re.captures(&content) {
        let list = &cap[1];
        for s in string_re.captures_iter(list) {
            let pattern = &s[1];
            if pattern.ends_with("/*") {
                // Glob: packages/*
                let dir = root.join(pattern.trim_end_matches("/*"));
                if dir.exists() {
                    for entry in std::fs::read_dir(&dir)? {
                        let entry = entry?;
                        if entry.path().is_dir() {
                            shared_paths.push(entry.path());
                        }
                    }
                }
            } else {
                let full = root.join(pattern);
                if full.exists() {
                    shared_paths.push(full);
                }
            }
        }
    }

    if app_paths.is_empty() {
        return Ok(None);
    }

    let mut workspace_apps = Vec::new();
    for app_path in &app_paths {
        let name = app_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let analysis = crate::analyzer::analyze(app_path)?;
        workspace_apps.push(WorkspaceApp {
            name,
            path: app_path.clone(),
            analysis,
        });
    }

    Ok(Some(Workspace {
        root: root.to_path_buf(),
        apps: workspace_apps,
        shared: shared_paths,
    }))
}

impl Workspace {
    /// Get total route count across all apps.
    pub fn total_routes(&self) -> usize {
        self.apps.iter().map(|a| a.analysis.routes.len()).sum()
    }

    /// Get a summary string.
    pub fn summary(&self) -> String {
        let app_names: Vec<&str> = self.apps.iter().map(|a| a.name.as_str()).collect();
        format!(
            "{} apps ({}), {} shared packages",
            self.apps.len(),
            app_names.join(", "),
            self.shared.len()
        )
    }
}
