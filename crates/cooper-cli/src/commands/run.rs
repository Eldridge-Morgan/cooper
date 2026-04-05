use anyhow::{Context, Result};
use colored::Colorize;
use cooper_codegen::analyzer::ProjectAnalysis;
use cooper_runtime::infra::embedded::{EmbeddedInfra, ServiceStatus};
use cooper_runtime::server::RuntimeServer;
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

pub async fn run(_all: bool, port: u16) -> Result<()> {
    let project_root = find_project_root()?;

    eprintln!("  {} Analyzing project...", "→".cyan());

    // Phase 1: Static analysis
    let analysis = cooper_codegen::analyzer::analyze(&project_root)
        .context("Failed to analyze project")?;

    print_analysis_summary(&analysis);

    // Phase 2: Start embedded infra
    eprintln!("  {} Starting local infrastructure...", "→".cyan());
    let mut infra = EmbeddedInfra::new(&project_root);
    let status = infra.start().await?;

    print_infra_status(&status);

    // Phase 3: Run migrations
    if analysis.has_databases() {
        eprintln!("  {} Running migrations...", "→".cyan());
        let mut total_migrations = 0u32;
        for db in &analysis.databases {
            if let Some(ref migrations_path) = db.migrations {
                let dir = project_root.join(migrations_path);
                match infra.run_migrations(&dir).await {
                    Ok(count) => total_migrations += count,
                    Err(e) => tracing::warn!("Migration error for db '{}': {}", db.name, e),
                }
            }
        }
        if total_migrations > 0 {
            eprintln!("  {} Ran {} migration(s)", "✓".green(), total_migrations);
        } else {
            eprintln!("  {} Migrations up to date", "✓".green());
        }
    }

    // Phase 4: Start the runtime server
    eprintln!("\n  {} Starting server on port {}...\n", "→".cyan(), port);

    let server = RuntimeServer::new(port, project_root.clone(), analysis.clone());
    let server = Arc::new(server);

    // Phase 5: Start cron scheduler
    if !analysis.crons.is_empty() {
        eprintln!(
            "  {} Scheduling {} cron job(s)",
            "⏰".to_string(),
            analysis.crons.len()
        );
        // Cron scheduler would need access to the JS runtime from the server
        // This is started after the server boots
    }

    // Phase 6: File watcher for hot reload
    let (tx, mut rx) = mpsc::channel::<()>(1);
    let _watcher_root = project_root.clone();
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            // Skip events in .cooper/, node_modules/, dist/
            let dominated_by_ignored = event.paths.iter().all(|p| {
                let s = p.to_string_lossy();
                s.contains("/.cooper/") || s.contains("/node_modules/") || s.contains("/dist/") || s.contains("/target/")
            });
            if dominated_by_ignored {
                return;
            }
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                    let _ = tx.blocking_send(());
                }
                _ => {}
            }
        }
    })?;
    watcher.watch(&project_root, RecursiveMode::Recursive)?;

    // Start server in background
    let server_handle = {
        let server = Arc::clone(&server);
        tokio::spawn(async move { server.start().await })
    };

    eprintln!(
        "  {} {} {}",
        "⚡".yellow(),
        "Cooper is running at".bold(),
        format!("http://localhost:{}", port).cyan().underline()
    );
    eprintln!(
        "  {} Dashboard at {}",
        "📊",
        "http://localhost:9400".cyan().underline()
    );
    eprintln!(
        "  {} API info at {}",
        "ℹ️",
        format!("http://localhost:{}/_cooper/info", port)
            .cyan()
            .underline()
    );
    eprintln!();

    // Hot reload loop
    let reload_server = Arc::clone(&server);
    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Debounce
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            while rx.try_recv().is_ok() {}

            eprintln!("  {} File changed — reloading...", "↻".yellow());
            if let Err(e) = reload_server.reload().await {
                eprintln!("  {} Reload failed: {}", "✗".red(), e);
            } else {
                eprintln!("  {} Reloaded", "✓".green());
            }
        }
    });

    // Wait for server to exit
    let result = server_handle.await?;

    // Cleanup
    infra.stop().await;

    result
}

fn find_project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let mut dir = cwd.as_path();
    loop {
        if dir.join("cooper.config.ts").exists() || dir.join("cooper.config.js").exists() {
            return Ok(dir.to_path_buf());
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => {
                return Err(anyhow::anyhow!(
                    "No cooper.config.ts found. Run `cooper new <name>` to create a project."
                ));
            }
        }
    }
}

fn print_analysis_summary(analysis: &ProjectAnalysis) {
    let mut parts = vec![format!("{} routes", analysis.routes.len())];
    if !analysis.topics.is_empty() {
        parts.push(format!("{} topics", analysis.topics.len()));
    }
    if !analysis.databases.is_empty() {
        parts.push(format!("{} databases", analysis.databases.len()));
    }
    if !analysis.crons.is_empty() {
        parts.push(format!("{} crons", analysis.crons.len()));
    }
    if !analysis.queues.is_empty() {
        parts.push(format!("{} queues", analysis.queues.len()));
    }
    if !analysis.pages.is_empty() {
        parts.push(format!("{} pages", analysis.pages.len()));
    }
    eprintln!("  {} Found {}", "✓".green(), parts.join(", "));
}

fn print_infra_status(status: &cooper_runtime::infra::embedded::InfraStatus) {
    print_service("Postgres", &status.postgres);
    print_service("NATS", &status.nats);
    print_service("Valkey", &status.valkey);
}

fn print_service(name: &str, status: &ServiceStatus) {
    match status {
        ServiceStatus::Running(port) => {
            eprintln!("  {} {} on port {}", "✓".green(), name, port);
        }
        ServiceStatus::External(port) => {
            eprintln!(
                "  {} {} (external) on port {}",
                "✓".green(),
                name,
                port
            );
        }
        ServiceStatus::InProcess => {
            eprintln!("  {} {} (in-process)", "✓".green(), name);
        }
        ServiceStatus::Unavailable(reason) => {
            eprintln!("  {} {} unavailable: {}", "⚠".yellow(), name, reason);
        }
    }
}
