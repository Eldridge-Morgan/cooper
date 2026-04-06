use anyhow::{Context, Result};
use colored::Colorize;
use cooper_codegen::analyzer::ProjectAnalysis;
use cooper_codegen::workspace;
use cooper_runtime::infra::embedded::{EmbeddedInfra, ServiceStatus};
use cooper_runtime::server::RuntimeServer;
use notify::{EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

pub async fn run(all: bool, port: u16) -> Result<()> {
    let project_root = find_project_root()?;

    // Check for monorepo
    if all {
        return run_workspace(project_root, port).await;
    }

    // Also auto-detect workspace if cooper.workspace.ts exists
    if project_root.join("cooper.workspace.ts").exists()
        || project_root.join("cooper.workspace.js").exists()
    {
        eprintln!(
            "  {} Workspace detected — use {} to run all apps",
            "ℹ".blue(),
            "cooper run --all".bold()
        );
    }

    run_single_app(project_root, port).await
}

/// Run a single Cooper app.
async fn run_single_app(project_root: PathBuf, port: u16) -> Result<()> {
    // Phase 0: Ensure Cooper SDK is available
    ensure_sdk(&project_root)?;

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

    // Inject infra connection URLs for JS workers
    // SAFETY: called before spawning worker threads, single-threaded at this point
    unsafe {
        if infra.pg_port > 0 {
            for db in &analysis.databases {
                let env_key = format!("COOPER_DB_{}_URL", db.name.to_uppercase());
                let url = format!(
                    "postgres://cooper@localhost:{}/cooper_{}",
                    infra.pg_port, db.name
                );
                std::env::set_var(&env_key, &url);
            }
        }
        if infra.valkey_port > 0 {
            std::env::set_var("COOPER_CACHE_URL", format!("redis://localhost:{}", infra.valkey_port));
        }
        if infra.nats_port > 0 {
            std::env::set_var("COOPER_NATS_URL", format!("nats://localhost:{}", infra.nats_port));
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
    }

    // Phase 6: File watcher for hot reload
    let (tx, mut rx) = mpsc::channel::<()>(1);
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            let dominated_by_ignored = event.paths.iter().all(|p| {
                let s = p.to_string_lossy();
                s.contains("/.cooper/")
                    || s.contains("/node_modules/")
                    || s.contains("/dist/")
                    || s.contains("/target/")
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
        "http://localhost:9500".cyan().underline()
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

    let result = server_handle.await?;
    infra.stop().await;
    result
}

/// Run all apps in a Cooper workspace.
async fn run_workspace(root: PathBuf, base_port: u16) -> Result<()> {
    eprintln!("  {} Detecting workspace...", "→".cyan());

    let ws = workspace::detect_workspace(&root)?
        .ok_or_else(|| anyhow::anyhow!("No workspace found. Create cooper.workspace.ts or add apps/ with cooper.config.ts files."))?;

    eprintln!("  {} {}", "✓".green(), ws.summary());

    // Ensure SDK in the root
    ensure_sdk(&root)?;

    // Start shared infra once
    eprintln!("  {} Starting shared infrastructure...", "→".cyan());
    let mut infra = EmbeddedInfra::new(&root);
    let status = infra.start().await?;
    print_infra_status(&status);

    // Run migrations from all apps
    eprintln!("  {} Running migrations...", "→".cyan());
    for app in &ws.apps {
        for db in &app.analysis.databases {
            if let Some(ref mig_path) = db.migrations {
                let dir = app.path.join(mig_path);
                if let Ok(count) = infra.run_migrations(&dir).await {
                    if count > 0 {
                        eprintln!(
                            "  {} {} — ran {} migration(s)",
                            "✓".green(),
                            app.name,
                            count
                        );
                    }
                }
            }
        }
    }

    // Start each app on its own port
    eprintln!();
    let mut handles = Vec::new();

    for (i, app) in ws.apps.iter().enumerate() {
        let app_port = base_port + i as u16;
        let app_name = app.name.clone();
        let app_path = app.path.clone();
        let analysis = app.analysis.clone();

        // Ensure SDK for each app
        ensure_sdk(&app_path)?;

        eprintln!(
            "  {} {} on port {}  ({} routes)",
            "⚡".yellow(),
            app_name.bold(),
            app_port.to_string().cyan(),
            analysis.routes.len()
        );

        let handle = tokio::spawn(async move {
            let server = RuntimeServer::new(app_port, app_path, analysis);
            if let Err(e) = server.start().await {
                tracing::error!("App '{}' failed: {}", app_name, e);
            }
        });
        handles.push(handle);
    }

    eprintln!();
    eprintln!(
        "  {} All {} apps running (ports {}–{})",
        "✓".green(),
        ws.apps.len(),
        base_port,
        base_port + ws.apps.len() as u16 - 1
    );

    if !ws.shared.is_empty() {
        let shared_names: Vec<String> = ws
            .shared
            .iter()
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect();
        eprintln!(
            "  {} Shared: {}",
            "📦".to_string(),
            shared_names.join(", ").dimmed()
        );
    }
    eprintln!();

    // File watcher across the whole workspace
    let (tx, mut rx) = mpsc::channel::<()>(1);
    let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
        if let Ok(event) = res {
            let ignored = event.paths.iter().all(|p| {
                let s = p.to_string_lossy();
                s.contains("/.cooper/")
                    || s.contains("/node_modules/")
                    || s.contains("/dist/")
                    || s.contains("/target/")
            });
            if ignored {
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
    watcher.watch(&root, RecursiveMode::Recursive)?;

    // Hot reload — detect which app changed and reload only that one
    tokio::spawn(async move {
        while rx.recv().await.is_some() {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            while rx.try_recv().is_ok() {}
            eprintln!("  {} Workspace file changed — reload not yet wired for multi-app", "↻".yellow());
        }
    });

    // Wait for any app to exit
    for handle in handles {
        let _ = handle.await;
    }

    infra.stop().await;
    Ok(())
}

/// Ensure the Cooper SDK is available in node_modules/.
fn ensure_sdk(project_root: &PathBuf) -> Result<()> {
    let sdk_source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sdk/src");

    // Check if cooper-stack (public npm) is installed — preferred
    let nm_stack = project_root.join("node_modules/cooper-stack");
    if nm_stack.join("package.json").exists() {
        return Ok(());
    }

    // Check if cooper (legacy) is already there
    let nm_cooper = project_root.join("node_modules/cooper");
    if nm_cooper.join("package.json").exists() {
        let pkg_content =
            std::fs::read_to_string(nm_cooper.join("package.json")).unwrap_or_default();
        if pkg_content.contains("\"description\": \"The backend framework for TypeScript\"") {
            return Ok(());
        }
        std::fs::remove_dir_all(&nm_cooper)?;
    }

    // Check if the scoped package is installed (@eldridge-morgan/cooper)
    let scoped_path = project_root.join("node_modules/@eldridge-morgan/cooper");
    if scoped_path.join("package.json").exists() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(&scoped_path, &nm_cooper)?;
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(&scoped_path, &nm_cooper)?;
        return Ok(());
    }

    std::fs::create_dir_all(nm_cooper.join("dist"))?;

    if sdk_source.exists() {
        let pkg = serde_json::json!({
            "name": "cooper",
            "version": "0.1.0",
            "description": "The backend framework for TypeScript",
            "type": "module",
            "main": "dist/index.js",
            "exports": {
                ".": "./dist/index.js",
                "./api": "./dist/api.js",
                "./db": "./dist/db.js",
                "./middleware": "./dist/middleware.js",
                "./auth": "./dist/auth.js",
                "./pubsub": "./dist/pubsub.js",
                "./cron": "./dist/cron.js",
                "./cache": "./dist/cache.js",
                "./storage": "./dist/storage.js",
                "./secrets": "./dist/secrets.js",
                "./queue": "./dist/queue.js",
                "./ssr": "./dist/ssr.js",
                "./islands": "./dist/islands.js",
                "./ai": "./dist/ai.js"
            }
        });
        std::fs::write(
            nm_cooper.join("package.json"),
            serde_json::to_string_pretty(&pkg)?,
        )?;

        for entry in std::fs::read_dir(&sdk_source)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "ts").unwrap_or(false) {
                let stem = path.file_stem().unwrap().to_string_lossy();
                let dest = nm_cooper.join("dist").join(format!("{stem}.js"));

                let relative = pathdiff::diff_paths(&path, nm_cooper.join("dist"))
                    .unwrap_or_else(|| path.clone());
                let relative_str = relative.to_string_lossy().replace('\\', "/");

                std::fs::write(&dest, format!("export * from \"{relative_str}\";\n"))?;
            }
        }
    }

    Ok(())
}

fn find_project_root() -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let mut dir = cwd.as_path();
    loop {
        if dir.join("cooper.config.ts").exists()
            || dir.join("cooper.config.js").exists()
            || dir.join("cooper.workspace.ts").exists()
            || dir.join("cooper.workspace.js").exists()
        {
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
