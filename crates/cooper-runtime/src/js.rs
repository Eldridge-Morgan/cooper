use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

/// The JavaScript runtime bridge.
///
/// In the full implementation, this embeds Deno Core (V8) to execute
/// TypeScript handlers. For now, it provides the interface that the
/// router and server depend on.
pub struct JsRuntime {
    project_root: PathBuf,
}

impl JsRuntime {
    pub fn new(project_root: PathBuf) -> Self {
        Self { project_root }
    }

    /// Call a handler function exported from a TypeScript source file.
    ///
    /// `source_file` — relative path like "services/users/api.ts"
    /// `export_name` — the named export, e.g. "getUser"
    /// `input` — JSON input (merged path params + body)
    ///
    /// Returns the JSON-serialized response from the handler.
    pub async fn call_handler(
        &self,
        source_file: &str,
        export_name: &str,
        input: &Value,
    ) -> Result<String> {
        let _full_path = self.project_root.join(source_file);

        // TODO: Full V8/Deno Core integration
        // For now, return a placeholder showing the routing works
        let response = serde_json::json!({
            "_cooper_debug": true,
            "handler": export_name,
            "source": source_file,
            "input": input,
            "message": "JS runtime not yet connected — handler was routed correctly"
        });

        Ok(serde_json::to_string(&response)?)
    }
}
