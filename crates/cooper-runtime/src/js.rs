use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, oneshot};

/// Number of JS worker processes to spawn
const WORKER_POOL_SIZE: usize = 4;

#[derive(Debug, Serialize)]
struct RpcRequest {
    id: u64,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct RpcResponse {
    id: u64,
    result: Option<Value>,
    error: Option<RpcError>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RpcError {
    pub code: String,
    pub message: String,
    #[serde(rename = "statusCode")]
    pub status_code: Option<u16>,
}

struct Worker {
    stdin: tokio::process::ChildStdin,
    pending: Arc<Mutex<std::collections::HashMap<u64, oneshot::Sender<RpcResponse>>>>,
    #[allow(dead_code)]
    child: Child,
}

pub struct JsRuntime {
    project_root: PathBuf,
    workers: Vec<Arc<Mutex<Worker>>>,
    next_worker: AtomicU64,
    request_id: AtomicU64,
}

impl JsRuntime {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            workers: Vec::new(),
            next_worker: AtomicU64::new(0),
            request_id: AtomicU64::new(1),
        }
    }

    /// Start the worker pool. Must be called before handling requests.
    pub async fn start(&mut self) -> Result<()> {
        let runtime = find_js_runtime()?;
        let bridge_path = self.resolve_bridge_path()?;

        for i in 0..WORKER_POOL_SIZE {
            let worker = spawn_worker(&runtime, &bridge_path, &self.project_root).await
                .with_context(|| format!("Failed to spawn JS worker {i}"))?;
            self.workers.push(Arc::new(Mutex::new(worker)));
        }

        tracing::info!("Started {} JS workers using {}", WORKER_POOL_SIZE, runtime);
        Ok(())
    }

    /// Call a handler function in a JS worker.
    pub async fn call_handler(
        &self,
        source_file: &str,
        export_name: &str,
        input: &Value,
    ) -> Result<String> {
        self.call_handler_with_auth(source_file, export_name, input, None, None).await
    }

    /// Call a handler with auth token and headers.
    pub async fn call_handler_with_auth(
        &self,
        source_file: &str,
        export_name: &str,
        input: &Value,
        auth_token: Option<&str>,
        headers: Option<&std::collections::HashMap<String, String>>,
    ) -> Result<String> {
        let mut params = serde_json::json!({
            "source": source_file,
            "export": export_name,
            "input": input,
        });

        if let Some(token) = auth_token {
            params["auth"] = serde_json::json!({ "token": token });
        }
        if let Some(hdrs) = headers {
            params["headers"] = serde_json::to_value(hdrs)?;
        }

        let response = self.rpc_call("call", params).await?;

        match response {
            RpcResult::Success(val) => Ok(serde_json::to_string(&val)?),
            RpcResult::Error(err) => {
                // Return the error as a JSON string so the router can parse it
                let err_json = serde_json::json!({
                    "error": {
                        "code": err.code,
                        "message": err.message,
                    }
                });
                Err(anyhow::anyhow!("{}", serde_json::to_string(&err_json)?))
            }
        }
    }

    /// Invalidate all module caches (for hot reload).
    pub async fn invalidate(&self) -> Result<()> {
        for worker in &self.workers {
            let id = self.request_id.fetch_add(1, Ordering::SeqCst);
            let req = RpcRequest {
                id,
                method: "invalidate".to_string(),
                params: Value::Null,
            };

            let mut w = worker.lock().await;
            let line = serde_json::to_string(&req)? + "\n";
            w.stdin.write_all(line.as_bytes()).await?;
        }
        Ok(())
    }

    /// Execute a cron job handler.
    pub async fn call_cron(&self, source_file: &str, export_name: &str) -> Result<()> {
        let params = serde_json::json!({
            "source": source_file,
            "export": export_name,
        });
        self.rpc_call("cron", params).await?;
        Ok(())
    }

    /// Deliver a pub/sub message to a subscriber.
    pub async fn deliver_pubsub(
        &self,
        topic: &str,
        subscriber: &str,
        data: &Value,
    ) -> Result<()> {
        let params = serde_json::json!({
            "topic": topic,
            "subscriber": subscriber,
            "data": data,
        });
        self.rpc_call("pubsub", params).await?;
        Ok(())
    }

    async fn rpc_call(&self, method: &str, params: Value) -> Result<RpcResult> {
        if self.workers.is_empty() {
            // Fallback for when workers haven't started yet
            return Ok(RpcResult::Success(serde_json::json!({
                "_cooper_debug": true,
                "message": "JS workers not started — call runtime.start() first",
            })));
        }

        let worker_idx =
            self.next_worker.fetch_add(1, Ordering::SeqCst) as usize % self.workers.len();
        let worker = &self.workers[worker_idx];

        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let req = RpcRequest {
            id,
            method: method.to_string(),
            params,
        };

        let (tx, rx) = oneshot::channel();

        {
            let mut w = worker.lock().await;
            w.pending.lock().await.insert(id, tx);
            let line = serde_json::to_string(&req)? + "\n";
            w.stdin.write_all(line.as_bytes()).await?;
        }

        let response = tokio::time::timeout(std::time::Duration::from_secs(30), rx)
            .await
            .map_err(|_| anyhow::anyhow!("JS handler timed out after 30s"))?
            .map_err(|_| anyhow::anyhow!("JS worker channel closed"))?;

        if let Some(err) = response.error {
            Ok(RpcResult::Error(err))
        } else {
            Ok(RpcResult::Success(response.result.unwrap_or(Value::Null)))
        }
    }

    fn resolve_bridge_path(&self) -> Result<PathBuf> {
        // Look for the bridge in several places:
        // 1. node_modules/cooperdev/dist/bridge.js (public npm)
        // 2. node_modules/cooper/dist/bridge.js (legacy/symlink)
        // 3. node_modules/@eldridge-morgan/cooper/dist/bridge.js (scoped)
        // 4. Bundled alongside the cooper binary
        let stack_path = self
            .project_root
            .join("node_modules/cooper-stack/dist/bridge.js");
        if stack_path.exists() {
            return Ok(stack_path);
        }

        let nm_path = self
            .project_root
            .join("node_modules/cooper/dist/bridge.js");
        if nm_path.exists() {
            return Ok(nm_path);
        }

        let scoped_path = self
            .project_root
            .join("node_modules/@eldridge-morgan/cooper/dist/bridge.js");
        if scoped_path.exists() {
            return Ok(scoped_path);
        }

        // Fallback: bundled bridge next to the binary
        let exe = std::env::current_exe()?;
        let bundled = exe.parent().unwrap().join("bridge.js");
        if bundled.exists() {
            return Ok(bundled);
        }

        // Development: use SDK source directly
        let dev_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../sdk/src/bridge.ts");
        if dev_path.exists() {
            return Ok(dev_path);
        }

        Err(anyhow::anyhow!(
            "Cooper bridge not found. Run `npm install cooper` or ensure the SDK is available."
        ))
    }
}

enum RpcResult {
    Success(Value),
    Error(RpcError),
}

/// Find a JS runtime: prefer Bun > Deno > Node
fn find_js_runtime() -> Result<String> {
    for runtime in &["bun", "deno", "node"] {
        if which::which(runtime).is_ok() {
            return Ok(runtime.to_string());
        }
    }
    Err(anyhow::anyhow!(
        "No JavaScript runtime found. Install Bun (recommended), Deno, or Node.js."
    ))
}

async fn spawn_worker(
    runtime: &str,
    bridge_path: &PathBuf,
    project_root: &PathBuf,
) -> Result<Worker> {
    let mut cmd = Command::new(runtime);

    // Runtime-specific flags
    match runtime {
        "bun" => {
            cmd.arg("run");
        }
        "deno" => {
            cmd.arg("run")
                .arg("--allow-all")
                .arg("--unstable");
        }
        "node" => {
            // For .ts files, use tsx or ts-node loader
            if bridge_path.extension().map(|e| e == "ts").unwrap_or(false) {
                cmd.arg("--import").arg("tsx");
            }
        }
        _ => {}
    }

    cmd.arg(bridge_path);
    cmd.current_dir(project_root);
    cmd.env("COOPER_PROJECT_ROOT", project_root);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::inherit()); // let worker errors show in console

    let mut child = cmd.spawn().context("Failed to spawn JS worker process")?;

    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let pending: Arc<Mutex<std::collections::HashMap<u64, oneshot::Sender<RpcResponse>>>> =
        Arc::new(Mutex::new(std::collections::HashMap::new()));

    // Spawn a reader task that routes responses to pending callers
    let pending_clone = Arc::clone(&pending);
    tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<RpcResponse>(&line) {
                Ok(response) => {
                    let mut pending = pending_clone.lock().await;
                    if let Some(tx) = pending.remove(&response.id) {
                        let _ = tx.send(response);
                    }
                    // id=0 responses are system messages (ready signal, etc.)
                }
                Err(e) => {
                    tracing::warn!("Failed to parse worker response: {e}: {line}");
                }
            }
        }
    });

    // Wait for the ready signal
    // (The reader task above will consume it, but since id=0 has no pending sender, it's fine)
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    Ok(Worker {
        stdin,
        pending,
        child,
    })
}
