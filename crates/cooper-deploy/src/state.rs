use crate::DeployResult;
use anyhow::Result;
use std::path::Path;

/// Load the deploy state for an environment.
pub fn load_state(env: &str) -> Result<Option<DeployResult>> {
    let path = format!(".cooper/state/{env}/deploy.json");
    if !Path::new(&path).exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path)?;
    let result: DeployResult = serde_json::from_str(&contents)?;
    Ok(Some(result))
}

/// List all known environments.
pub fn list_environments() -> Result<Vec<String>> {
    let state_dir = ".cooper/state";
    if !Path::new(state_dir).exists() {
        return Ok(vec![]);
    }

    let mut envs = Vec::new();
    for entry in std::fs::read_dir(state_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                envs.push(name.to_string());
            }
        }
    }
    Ok(envs)
}
