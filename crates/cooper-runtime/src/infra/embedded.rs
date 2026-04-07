use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

/// Manages local development infrastructure via Docker Compose.
/// Starts Postgres, NATS (with JetStream), and Valkey containers.
pub struct EmbeddedInfra {
    pub pg_port: u16,
    pub nats_port: u16,
    pub valkey_port: u16,
    data_dir: PathBuf,
    compose_file: PathBuf,
}

impl EmbeddedInfra {
    pub fn new(project_root: &Path) -> Self {
        let data_dir = project_root.join(".cooper/data");
        let compose_file = data_dir.join("docker-compose.yml");
        Self {
            pg_port: 5432,
            nats_port: 4222,
            valkey_port: 6379,
            data_dir,
            compose_file,
        }
    }

    /// Start all infrastructure via Docker Compose.
    pub async fn start(&mut self) -> Result<InfraStatus> {
        std::fs::create_dir_all(&self.data_dir)?;

        let mut status = InfraStatus::default();

        // Check Docker is available
        if !docker_available().await {
            return Err(anyhow::anyhow!(
                "Docker is required for local development.\n  Install: https://docs.docker.com/get-docker/"
            ));
        }

        // Find free ports
        self.pg_port = find_free_port().await?;
        self.nats_port = find_free_port().await?;
        self.valkey_port = find_free_port().await?;

        // Generate docker-compose.yml
        let compose = generate_compose(self.pg_port, self.nats_port, self.valkey_port);
        std::fs::write(&self.compose_file, &compose)?;

        // Start containers
        let output = Command::new("docker")
            .args(["compose", "-f", self.compose_file.to_str().unwrap(), "up", "-d", "--wait"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Try `docker-compose` (v1) if `docker compose` (v2) fails
            let output_v1 = Command::new("docker-compose")
                .args(["-f", self.compose_file.to_str().unwrap(), "up", "-d"])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await;

            match output_v1 {
                Ok(o) if o.status.success() => {}
                _ => {
                    return Err(anyhow::anyhow!(
                        "Failed to start Docker containers: {}",
                        stderr.trim()
                    ));
                }
            }
        }

        // Wait for services to be ready
        let timeout = std::time::Duration::from_secs(30);
        let start = std::time::Instant::now();

        // Wait for Postgres
        while start.elapsed() < timeout {
            if check_port_open(self.pg_port).await {
                status.postgres = ServiceStatus::Running(self.pg_port);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if !matches!(status.postgres, ServiceStatus::Running(_)) {
            status.postgres = ServiceStatus::Unavailable("Postgres container not ready".to_string());
        }

        // Wait for NATS
        while start.elapsed() < timeout {
            if check_port_open(self.nats_port).await {
                status.nats = ServiceStatus::Running(self.nats_port);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if !matches!(status.nats, ServiceStatus::Running(_)) {
            status.nats = ServiceStatus::Unavailable("NATS container not ready".to_string());
        }

        // Wait for Valkey
        while start.elapsed() < timeout {
            if check_port_open(self.valkey_port).await {
                status.valkey = ServiceStatus::Running(self.valkey_port);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        if !matches!(status.valkey, ServiceStatus::Running(_)) {
            status.valkey = ServiceStatus::Unavailable("Valkey container not ready".to_string());
        }

        // Create the cooper_main database (Postgres is ready, create db if not exists)
        if matches!(status.postgres, ServiceStatus::Running(_)) {
            // Wait a bit more for Postgres to accept connections
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = Command::new("docker")
                .args([
                    "compose", "-f", self.compose_file.to_str().unwrap(),
                    "exec", "-T", "postgres",
                    "createdb", "-U", "cooper", "cooper_main",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }

        Ok(status)
    }

    /// Run SQL migration files against Postgres.
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
        for entry in &files {
            let sql = std::fs::read_to_string(entry.path())?;
            let status = Command::new("docker")
                .args([
                    "compose", "-f", self.compose_file.to_str().unwrap(),
                    "exec", "-T", "postgres",
                    "psql", "-U", "cooper", "-d", "cooper_main", "-c", &sql,
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

    /// Stop all containers.
    pub async fn stop(&mut self) {
        if self.compose_file.exists() {
            let _ = Command::new("docker")
                .args(["compose", "-f", self.compose_file.to_str().unwrap(), "down"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
    }
}

impl Drop for EmbeddedInfra {
    fn drop(&mut self) {
        // Best-effort cleanup — stop containers
        if self.compose_file.exists() {
            let _ = std::process::Command::new("docker")
                .args(["compose", "-f", self.compose_file.to_str().unwrap(), "down"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
}

fn generate_compose(pg_port: u16, nats_port: u16, valkey_port: u16) -> String {
    format!(
        r#"# Auto-generated by Cooper — do not edit
services:
  postgres:
    image: postgres:17-alpine
    environment:
      POSTGRES_USER: cooper
      POSTGRES_HOST_AUTH_METHOD: trust
    ports:
      - "{pg_port}:5432"
    volumes:
      - cooper-pg-data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U cooper"]
      interval: 1s
      timeout: 3s
      retries: 10

  nats:
    image: nats:2.10-alpine
    command: ["--jetstream", "--store_dir", "/data"]
    ports:
      - "{nats_port}:4222"
    volumes:
      - cooper-nats-data:/data
    healthcheck:
      test: ["CMD", "nats-server", "--help"]
      interval: 1s
      timeout: 3s
      retries: 10

  valkey:
    image: valkey/valkey:8-alpine
    ports:
      - "{valkey_port}:6379"
    healthcheck:
      test: ["CMD", "valkey-cli", "ping"]
      interval: 1s
      timeout: 3s
      retries: 10

volumes:
  cooper-pg-data:
  cooper-nats-data:
"#
    )
}

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

async fn find_free_port() -> Result<u16> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    drop(listener);
    Ok(port)
}

async fn check_port_open(port: u16) -> bool {
    tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .is_ok()
}
