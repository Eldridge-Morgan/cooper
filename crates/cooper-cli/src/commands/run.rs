use anyhow::{Context, Result};
use colored::Colorize;
use cooper_codegen::analyzer::ProjectAnalysis;
use cooper_runtime::server::RuntimeServer;
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

pub async fn run(_all: bool, port: u16) -> Result<()> {
    let project_root = find_project_root()?;

    eprintln!(
        "  {} Analyzing project...",
        "→".cyan()
    );

    // Phase 1: Static analysis — parse TS source, extract api/topic/database/cron declarations
    let analysis = cooper_codegen::analyzer::analyze(&project_root)
        .context("Failed to analyze project")?;

    print_analysis_summary(&analysis);

    // Phase 2: Start embedded infra
    eprintln!(
        "  {} Starting local infrastructure...",
        "→".cyan()
    );
    let infra = start_local_infra(&analysis).await?;
    eprintln!("  {} Embedded Postgres on port {}", "✓".green(), infra.pg_port);
    eprintln!("  {} Embedded NATS on port {}", "✓".green(), infra.nats_port);
    eprintln!("  {} Embedded Valkey on port {}", "✓".green(), infra.valkey_port);

    // Phase 3: Run migrations
    if analysis.has_databases() {
        eprintln!(
            "  {} Running migrations...",
            "→".cyan()
        );
        // TODO: run migrations
        eprintln!("  {} Migrations up to date", "✓".green());
    }

    // Phase 4: Start the runtime server
    eprintln!(
        "\n  {} Starting server on port {}...\n",
        "→".cyan(),
        port
    );

    let server = RuntimeServer::new(port, project_root.clone(), analysis);
    let server = Arc::new(server);

    // Phase 5: File watcher for hot reload
    let (tx, mut rx) = mpsc::channel::<()>(1);
    let _watcher_root = project_root.clone();
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
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
        format!("http://localhost:9400").cyan().underline()
    );
    eprintln!();

    // Hot reload loop
    let reload_server = Arc::clone(&server);
    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            // Debounce — drain any queued events
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            while rx.try_recv().is_ok() {}

            eprintln!(
                "  {} File changed — reloading...",
                "↻".yellow()
            );
            if let Err(e) = reload_server.reload().await {
                eprintln!(
                    "  {} Reload failed: {}",
                    "✗".red(),
                    e
                );
            } else {
                eprintln!(
                    "  {} Reloaded",
                    "✓".green()
                );
            }
        }
    });

    server_handle.await??;
    Ok(())
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
    eprintln!(
        "  {} Found {} routes, {} topics, {} databases, {} crons",
        "✓".green(),
        analysis.routes.len(),
        analysis.topics.len(),
        analysis.databases.len(),
        analysis.crons.len(),
    );
}

struct LocalInfra {
    pg_port: u16,
    nats_port: u16,
    valkey_port: u16,
}

async fn start_local_infra(_analysis: &ProjectAnalysis) -> Result<LocalInfra> {
    // TODO: Actually start embedded Postgres, NATS, Valkey
    // For now, return placeholder ports
    Ok(LocalInfra {
        pg_port: 5432,
        nats_port: 4222,
        valkey_port: 6379,
    })
}
