use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Scans the islands/ directory and collects all .island.tsx/.island.ts files.
pub struct IslandRegistry {
    islands: HashMap<String, PathBuf>,
}

impl IslandRegistry {
    pub fn new(project_root: &Path) -> Self {
        let mut islands = HashMap::new();

        let islands_dir = project_root.join("islands");
        if islands_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&islands_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();

                    // Strip .island suffix if present
                    let clean_name = name
                        .strip_suffix(".island")
                        .unwrap_or(&name)
                        .to_string();

                    if is_island_file(&path) {
                        islands.insert(clean_name, path);
                    }
                }
            }
        }

        Self { islands }
    }

    pub fn get(&self, name: &str) -> Option<&PathBuf> {
        self.islands.get(name)
    }

    pub fn list(&self) -> Vec<String> {
        self.islands.keys().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.islands.len()
    }
}

fn is_island_file(path: &Path) -> bool {
    let name = path.to_string_lossy();
    name.ends_with(".island.tsx")
        || name.ends_with(".island.ts")
        || name.ends_with(".island.jsx")
        || name.ends_with(".island.js")
}
