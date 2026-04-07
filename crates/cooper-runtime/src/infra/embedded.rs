use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

const PG_IMAGE: &str = "postgres:17-alpine";
const NATS_IMAGE: &str = "nats:2.10-alpine";
const VALKEY_IMAGE: &str = "valkey/valkey:8-alpine";

/// Manages local development infrastructure via Docker containers.
/// Each service gets a deterministic container name per project so containers
/// are reused across restarts (like Encore).
pub struct EmbeddedInfra {
    pub pg_port: u16,
    pub nats_port: u16,
    pub valkey_port: u16,
    project_id: String,
    data_dir: PathBuf,
}

impl EmbeddedInfra {
    pub fn new(project_root: &Path) -> Self {
        // Derive a stable project ID from the directory name
        let project_id = project_root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
            .to_lowercase();

        Self {
            pg_port: 0,
            nats_port: 0,
            valkey_port: 0,
            project_id,
            data_dir: project_root.join(".cooper/data"),
        }
    }

    fn container_name(&self, service: &str) -> String {
        format!("cooper-{}-{}", self.project_id, service)
    }

    fn volume_name(&self, service: &str) -> String {
        format!("cooper-{}-{}-data", self.project_id, service)
    }

    /// Start all infrastructure.
    /// Reuses running containers, restarts stopped ones, creates new if needed.
    pub async fn start(&mut self) -> Result<InfraStatus> {
        std::fs::create_dir_all(&self.data_dir)?;

        let mut status = InfraStatus::default();

        // Check Docker is available
        if !docker_available().await {
            return Err(anyhow::anyhow!(
                "Docker is required for local development.\n  Install: https://docs.docker.com/get-docker/"
            ));
        }

        // Start each service (reuse if already running)
        match self.ensure_container("postgres", PG_IMAGE, 5432, &[
            "-e", "POSTGRES_USER=cooper",
            "-e", "POSTGRES_HOST_AUTH_METHOD=trust",
            "-v", &format!("{}:/var/lib/postgresql/data", self.volume_name("pg")),
        ]).await {
            Ok(port) => {
                self.pg_port = port;
                // Wait for Postgres to actually accept connections
                if wait_for_postgres(port).await {
                    // Create cooper_main database if it doesn't exist
                    create_database(port, "cooper_main").await;
                    status.postgres = ServiceStatus::Running(port);
                } else {
                    status.postgres = ServiceStatus::Unavailable("Postgres not ready".into());
                }
            }
            Err(e) => status.postgres = ServiceStatus::Unavailable(e.to_string()),
        }

        match self.ensure_container("nats", NATS_IMAGE, 4222, &[
            "-v", &format!("{}:/data", self.volume_name("nats")),
            // NATS command args passed after image
        ]).await {
            Ok(port) => {
                self.nats_port = port;
                if wait_for_port(port, 15).await {
                    status.nats = ServiceStatus::Running(port);
                } else {
                    status.nats = ServiceStatus::Unavailable("NATS not ready".into());
                }
            }
            Err(e) => status.nats = ServiceStatus::Unavailable(e.to_string()),
        }

        match self.ensure_container("valkey", VALKEY_IMAGE, 6379, &[
        ]).await {
            Ok(port) => {
                self.valkey_port = port;
                if wait_for_port(port, 15).await {
                    status.valkey = ServiceStatus::Running(port);
                } else {
                    status.valkey = ServiceStatus::Unavailable("Valkey not ready".into());
                }
            }
            Err(e) => status.valkey = ServiceStatus::Unavailable(e.to_string()),
        }

        Ok(status)
    }

    /// Ensure a container is running. Returns the host port.
    /// Three states: Running → return port, Stopped → start + return port, NotFound → create.
    async fn ensure_container(
        &self,
        service: &str,
        image: &str,
        container_port: u16,
        extra_args: &[&str],
    ) -> Result<u16> {
        let name = self.container_name(service);

        // Check container state
        match inspect_container(&name).await {
            ContainerState::Running(port) => {
                tracing::debug!("Container {name} already running on port {port}");
                return Ok(port);
            }
            ContainerState::Stopped => {
                // Restart it
                let output = Command::new("docker")
                    .args(["start", &name])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await?;

                if !output.success() {
                    // Remove and recreate
                    let _ = Command::new("docker").args(["rm", "-f", &name]).stdout(Stdio::null()).stderr(Stdio::null()).status().await;
                } else {
                    // Get the port after restart
                    if let ContainerState::Running(port) = inspect_container(&name).await {
                        return Ok(port);
                    }
                }
            }
            ContainerState::NotFound => {}
        }

        // Create new container
        let mut args = vec![
            "run".to_string(), "-d".to_string(),
            "--name".to_string(), name.clone(),
            "-p".to_string(), format!("0:{container_port}"), // Docker assigns random host port
        ];

        for arg in extra_args {
            args.push(arg.to_string());
        }

        args.push(image.to_string());

        // Add NATS-specific command args after image
        if service == "nats" {
            args.extend(["--jetstream".to_string(), "--store_dir".to_string(), "/data".to_string()]);
        }

        let output = Command::new("docker")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create {service} container: {}", stderr.trim()));
        }

        // Read the assigned port
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        match inspect_container(&name).await {
            ContainerState::Running(port) => Ok(port),
            _ => Err(anyhow::anyhow!("Container {name} created but not running")),
        }
    }

    /// Run SQL migration files against Postgres via direct TCP connection.
    pub async fn run_migrations(&self, migrations_dir: &Path) -> Result<u32> {
        if !migrations_dir.exists() {
            return Ok(0);
        }

        let mut files: Vec<_> = std::fs::read_dir(migrations_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .map(|ext| ext == "sql")
                    .unwrap_or(false)
            })
            .collect();
        files.sort_by_key(|e| e.file_name());

        let mut count = 0u32;
        let name = self.container_name("postgres");

        for entry in &files {
            let sql = std::fs::read_to_string(entry.path())?;
            // Use docker exec with psql inside the container
            let status = Command::new("docker")
                .args([
                    "exec", "-i", &name,
                    "psql", "-U", "cooper", "-d", "cooper_main",
                    "-c", &sql,
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;

            if let Ok(s) = status {
                if s.success() {
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Stop all containers (keep volumes for data persistence).
    pub async fn stop(&mut self) {
        for service in &["postgres", "nats", "valkey"] {
            let name = self.container_name(service);
            let _ = Command::new("docker")
                .args(["stop", &name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
    }
}

impl Drop for EmbeddedInfra {
    fn drop(&mut self) {
        // Stop containers but keep volumes (data persists)
        for service in &["postgres", "nats", "valkey"] {
            let name = self.container_name(service);
            let _ = std::process::Command::new("docker")
                .args(["stop", &name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

// --- Container inspection ---

enum ContainerState {
    Running(u16),
    Stopped,
    NotFound,
}

async fn inspect_container(name: &str) -> ContainerState {
    let output = Command::new("docker")
        .args(["inspect", "--format", "{{.State.Running}}|{{json .NetworkSettings.Ports}}", name])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await;

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return ContainerState::NotFound,
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();

    let parts: Vec<&str> = stdout.splitn(2, '|').collect();
    if parts.len() != 2 {
        return ContainerState::NotFound;
    }

    let running = parts[0] == "true";
    if !running {
        return ContainerState::Stopped;
    }

    // Parse port mapping from JSON like {"5432/tcp":[{"HostIp":"0.0.0.0","HostPort":"55123"}]}
    if let Ok(ports) = serde_json::from_str::<serde_json::Value>(parts[1]) {
        if let Some(obj) = ports.as_object() {
            for (_key, bindings) in obj {
                if let Some(arr) = bindings.as_array() {
                    for binding in arr {
                        if let Some(port_str) = binding.get("HostPort").and_then(|v| v.as_str()) {
                            if let Ok(port) = port_str.parse::<u16>() {
                                if port > 0 {
                                    return ContainerState::Running(port);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    ContainerState::Stopped
}

// --- Database creation via TCP ---

/// Wait for Postgres to accept real connections (wire protocol check, not just port).
async fn wait_for_postgres(port: u16) -> bool {
    for _ in 0..60 {
        // Try connecting with psql via Docker
        let result = Command::new("docker")
            .args(["run", "--rm", "--network=host", PG_IMAGE,
                   "pg_isready", "-h", "localhost", "-p", &port.to_string(), "-U", "cooper"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        if let Ok(s) = result {
            if s.success() {
                return true;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    false
}

/// Create a database if it doesn't exist.
async fn create_database(port: u16, db_name: &str) {
    // Use the running postgres container to create the database
    // psql inside the container connects via localhost
    let check = Command::new("docker")
        .args(["run", "--rm", "--network=host", PG_IMAGE,
               "psql", "-h", "localhost", "-p", &port.to_string(),
               "-U", "cooper", "-d", "postgres",
               "-tAc", &format!("SELECT 1 FROM pg_database WHERE datname='{db_name}'")])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await;

    let exists = check
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().contains('1'))
        .unwrap_or(false);

    if !exists {
        let _ = Command::new("docker")
            .args(["run", "--rm", "--network=host", PG_IMAGE,
                   "createdb", "-h", "localhost", "-p", &port.to_string(),
                   "-U", "cooper", db_name])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    }
}

async fn wait_for_port(port: u16, timeout_secs: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
    while std::time::Instant::now() < deadline {
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{port}")).await.is_ok() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    false
}

async fn docker_available() -> bool {
    Command::new("docker")
        .args(["info"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

// --- Status types ---

#[derive(Default)]
pub struct InfraStatus {
    pub postgres: ServiceStatus,
    pub nats: ServiceStatus,
    pub valkey: ServiceStatus,
}

pub enum ServiceStatus {
    Running(u16),
    External(u16),
    InProcess,
    Unavailable(String),
}

impl Default for ServiceStatus {
    fn default() -> Self {
        ServiceStatus::Unavailable("not started".to_string())
    }
}

impl ServiceStatus {
    #[allow(dead_code)]
    pub fn port(&self) -> Option<u16> {
        match self {
            ServiceStatus::Running(p) | ServiceStatus::External(p) => Some(*p),
            _ => None,
        }
    }

    pub fn display(&self) -> String {
        match self {
            ServiceStatus::Running(p) => format!("docker on port {p}"),
            ServiceStatus::External(p) => format!("external on port {p}"),
            ServiceStatus::InProcess => "in-process".to_string(),
            ServiceStatus::Unavailable(reason) => format!("unavailable ({reason})"),
        }
    }

    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        !matches!(self, ServiceStatus::Unavailable(_))
    }
}
