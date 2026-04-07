use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::{Child, Command};

use super::binaries::resolve_binary;

/// Manages embedded infrastructure for local development.
pub struct EmbeddedInfra {
    pub pg_port: u16,
    pub nats_port: u16,
    pub valkey_port: u16,
    pg_process: Option<Child>,
    nats_process: Option<Child>,
    valkey_process: Option<Child>,
    data_dir: PathBuf,
}

impl EmbeddedInfra {
    pub fn new(project_root: &Path) -> Self {
        Self {
            pg_port: 5432,
            nats_port: 4222,
            valkey_port: 6379,
            pg_process: None,
            nats_process: None,
            valkey_process: None,
            data_dir: project_root.join(".cooper/data"),
        }
    }

    /// Start all embedded infrastructure.
    /// Falls back gracefully if binaries are not installed.
    /// Each service has a timeout — we never block the server from starting.
    pub async fn start(&mut self) -> Result<InfraStatus> {
        std::fs::create_dir_all(&self.data_dir)?;

        let mut status = InfraStatus::default();

        let timeout = std::time::Duration::from_secs(10);

        // Postgres
        match tokio::time::timeout(timeout, self.start_postgres()).await {
            Ok(Ok(port)) => {
                self.pg_port = port;
                status.postgres = ServiceStatus::Running(port);
            }
            Ok(Err(e)) => {
                // Only use external Postgres if we can actually connect to it
                // with the cooper role. A random system Postgres on 5432 with
                // SCRAM auth and no cooper role will fail at query time.
                if check_port_open(5432).await && verify_postgres(5432).await {
                    self.pg_port = 5432;
                    status.postgres = ServiceStatus::External(5432);
                } else {
                    status.postgres = ServiceStatus::Unavailable(e.to_string());
                }
            }
            Err(_) => {
                status.postgres = ServiceStatus::Unavailable("timed out".to_string());
            }
        }

        // NATS
        match tokio::time::timeout(timeout, self.start_nats()).await {
            Ok(Ok(port)) => {
                self.nats_port = port;
                status.nats = ServiceStatus::Running(port);
            }
            _ => {
                if check_port_open(4222).await {
                    self.nats_port = 4222;
                    status.nats = ServiceStatus::External(4222);
                } else {
                    status.nats = ServiceStatus::InProcess;
                }
            }
        }

        // Valkey/Redis
        match tokio::time::timeout(timeout, self.start_valkey()).await {
            Ok(Ok(port)) => {
                self.valkey_port = port;
                status.valkey = ServiceStatus::Running(port);
            }
            _ => {
                if check_port_open(6379).await {
                    self.valkey_port = 6379;
                    status.valkey = ServiceStatus::External(6379);
                } else {
                    status.valkey = ServiceStatus::InProcess;
                }
            }
        }

        Ok(status)
    }

    async fn start_postgres(&mut self) -> Result<u16> {
        let pg_dir = self.data_dir.join("postgres");
        std::fs::create_dir_all(&pg_dir)?;

        // Find a free port
        let port = find_free_port().await?;

        // Check if pg_ctl is available
        let pg_ctl = find_binary("pg_ctl")?;

        // Initialize if needed. We use a marker file to ensure the data dir
        // was initialized with our "cooper" superuser. If the marker is missing
        // (stale data dir from an older version), re-initialize.
        let data_path = pg_dir.join("data");
        let cooper_marker = data_path.join(".cooper_init");
        if data_path.join("PG_VERSION").exists() && !cooper_marker.exists() {
            tracing::info!("Reinitializing Postgres data dir (missing cooper role)");
            let _ = std::fs::remove_dir_all(&data_path);
        }
        if !data_path.join("PG_VERSION").exists() {
            let initdb = find_binary("initdb")?;
            let status = Command::new(&initdb)
                .args(["--pgdata", data_path.to_str().unwrap()])
                .args(["--auth", "trust"])
                .args(["--username", "cooper"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await?;
            if !status.success() {
                return Err(anyhow::anyhow!("initdb failed"));
            }
            // Mark this data dir as initialized with the cooper superuser
            let _ = std::fs::write(&cooper_marker, "cooper");
        }

        // Start Postgres (pg_ctl start -w waits until ready then exits)
        let status = Command::new(&pg_ctl)
            .args(["start", "-w", "-D", data_path.to_str().unwrap()])
            .args(["-o", &format!("-p {port} -k {}", pg_dir.to_str().unwrap())])
            .args(["-l", pg_dir.join("postgres.log").to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;

        if !status.success() {
            return Err(anyhow::anyhow!("pg_ctl start failed"));
        }

        // Wait for Postgres to be ready
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if check_port_open(port).await {
                // Create default database (connect via socket dir)
                let socket_dir = pg_dir.to_str().unwrap();
                let _ = Command::new(find_binary("createdb").unwrap_or_default())
                    .args(["-p", &port.to_string()])
                    .args(["-h", socket_dir])
                    .args(["-U", "cooper"])
                    .arg("cooper_main")
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await;
                return Ok(port);
            }
        }

        Err(anyhow::anyhow!("Postgres did not start in time"))
    }

    async fn start_nats(&mut self) -> Result<u16> {
        let nats_binary = resolve_binary("nats-server").await?;
        let port = find_free_port().await?;
        let store_dir = self.data_dir.join("nats");
        std::fs::create_dir_all(&store_dir)?;

        let child = Command::new(&nats_binary)
            .args(["-p", &port.to_string()])
            .args(["--jetstream"])
            .args(["--store_dir", store_dir.to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        self.nats_process = Some(child);

        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if check_port_open(port).await {
                return Ok(port);
            }
        }

        Err(anyhow::anyhow!("NATS did not start in time"))
    }

    async fn start_valkey(&mut self) -> Result<u16> {
        // Try valkey-server first, then redis-server (check PATH, then auto-download)
        let binary = match resolve_binary("valkey-server").await {
            Ok(b) => b,
            Err(_) => find_binary("redis-server")?,
        };
        let port = find_free_port().await?;
        let data_dir = self.data_dir.join("valkey");
        std::fs::create_dir_all(&data_dir)?;

        let child = Command::new(&binary)
            .args(["--port", &port.to_string()])
            .args(["--dir", data_dir.to_str().unwrap()])
            .args(["--save", ""])
            .args(["--daemonize", "no"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        self.valkey_process = Some(child);

        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if check_port_open(port).await {
                return Ok(port);
            }
        }

        Err(anyhow::anyhow!("Valkey/Redis did not start in time"))
    }

    /// Run SQL migration files against the embedded Postgres.
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
            let psql = find_binary("psql").unwrap_or_else(|_| "psql".into());

            let pg_dir = self.data_dir.join("postgres");
            let socket_dir = pg_dir.to_str().unwrap_or("/tmp");
            let status = Command::new(&psql)
                .args(["-p", &self.pg_port.to_string()])
                .args(["-h", socket_dir])
                .args(["-U", "cooper"])
                .args(["-d", "cooper_main"])
                .args(["-c", &sql])
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

    /// Stop all embedded infrastructure.
    pub async fn stop(&mut self) {
        if let Some(mut child) = self.pg_process.take() {
            let _ = child.kill().await;
        }
        if let Some(mut child) = self.nats_process.take() {
            let _ = child.kill().await;
        }
        if let Some(mut child) = self.valkey_process.take() {
            let _ = child.kill().await;
        }
    }
}

impl Drop for EmbeddedInfra {
    fn drop(&mut self) {
        // Best-effort cleanup
        if let Some(ref mut child) = self.pg_process {
            let _ = child.start_kill();
        }
        if let Some(ref mut child) = self.nats_process {
            let _ = child.start_kill();
        }
        if let Some(ref mut child) = self.valkey_process {
            let _ = child.start_kill();
        }
    }
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
            ServiceStatus::Running(p) => format!("embedded on port {p}"),
            ServiceStatus::External(p) => format!("external on port {p}"),
            ServiceStatus::InProcess => "in-process".to_string(),
            ServiceStatus::Unavailable(reason) => format!("unavailable ({reason})"),
        }
    }
}

// Two variants: one with a String, one with no data
// Fix the enum to have Unavailable take an optional string
impl ServiceStatus {
    #[allow(dead_code)]
    pub fn is_available(&self) -> bool {
        !matches!(self, ServiceStatus::Unavailable(_))
    }
}

fn find_binary(name: &str) -> Result<String> {
    which::which(name)
        .map(|p| p.to_string_lossy().to_string())
        .map_err(|_| anyhow::anyhow!("{name} not found in PATH"))
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

/// Verify that an external Postgres on the given port is usable by Cooper.
/// Tries to connect as the "cooper" user with no password and run a simple query.
/// Returns false if auth fails, role doesn't exist, or connection is refused.
async fn verify_postgres(port: u16) -> bool {
    let psql = match find_binary("psql") {
        Ok(p) => p,
        Err(_) => return false,
    };

    let result = Command::new(&psql)
        .args(["-p", &port.to_string()])
        .args(["-h", "localhost"])
        .args(["-U", "cooper"])
        .args(["-d", "postgres"])
        .args(["-c", "SELECT 1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await;

    match result {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}
