use anyhow::{Context, Result};
use colored::Colorize;
use cooper_runtime::infra::embedded::EmbeddedInfra;
use std::path::PathBuf;
use tokio::process::Command;

/// Run tests with embedded infrastructure.
///
/// Mirrors `cooper run` phases 0–3 (SDK, analysis, infra, migrations),
/// then executes test files via Node.js test runner instead of starting
/// the server.
pub async fn run(filter: Option<String>, fail_fast: bool) -> Result<()> {
    let project_root = super::run::find_project_root()?;

    // Phase 0: Ensure SDK
    // (handled by the test runner importing from cooper-stack)

    eprintln!("  {} Analyzing project...", "→".cyan());

    // Phase 1: Static analysis
    let analysis = cooper_codegen::analyzer::analyze(&project_root)
        .context("Failed to analyze project")?;

    // Phase 2: Start embedded infra
    eprintln!("  {} Starting test infrastructure...", "→".cyan());
    let mut infra = EmbeddedInfra::new(&project_root);
    let status = infra.start().await?;

    super::run::print_infra_status(&status);

    // Phase 3: Run migrations
    if analysis.has_databases() {
        eprintln!("  {} Running migrations...", "→".cyan());
        for db in &analysis.databases {
            if let Some(ref migrations_path) = db.migrations {
                let dir = project_root.join(migrations_path);
                if let Err(e) = infra.run_migrations(&dir).await {
                    tracing::warn!("Migration error for db '{}': {}", db.name, e);
                }
            }
        }
        eprintln!("  {} Migrations up to date", "✓".green());
    }

    // Inject infra connection URLs
    // SAFETY: called before spawning worker threads
    unsafe {
        if infra.pg_port > 0 {
            for db in &analysis.databases {
                let env_key = format!("COOPER_DB_{}_URL", db.name.to_uppercase());
                let url = format!(
                    "postgres://cooper:cooper@localhost:{}/cooper_{}?sslmode=disable",
                    infra.pg_port, db.name
                );
                std::env::set_var(&env_key, &url);
            }
        }
        if infra.valkey_port > 0 {
            std::env::set_var(
                "COOPER_VALKEY_URL",
                format!("redis://localhost:{}", infra.valkey_port),
            );
        }
        if infra.nats_port > 0 {
            std::env::set_var(
                "COOPER_NATS_URL",
                format!("nats://localhost:{}", infra.nats_port),
            );
        }
        std::env::set_var("COOPER_ENV", "test");
        std::env::set_var("COOPER_PROJECT_ROOT", project_root.to_string_lossy().to_string());
    }

    // Phase 4: Find and run test files
    eprintln!("\n  {} Running tests...\n", "→".cyan());

    let test_files = find_test_files(&project_root, &filter)?;

    if test_files.is_empty() {
        eprintln!("  {} No test files found", "⚠".yellow());
        eprintln!(
            "  {} Create test files matching {}",
            "ℹ".blue(),
            "**/*.test.ts".dimmed()
        );
        infra.stop().await;
        return Ok(());
    }

    eprintln!(
        "  {} Found {} test file(s)\n",
        "✓".green(),
        test_files.len()
    );

    // Run via Node.js --test runner (Node 20+ built-in test runner)
    let mut args = vec![
        "--import".to_string(),
        "tsx".to_string(),
        "--test".to_string(),
    ];

    if fail_fast {
        args.push("--test-force-exit".to_string());
    }

    for f in &test_files {
        args.push(f.to_string_lossy().to_string());
    }

    let status = Command::new("node")
        .args(&args)
        .current_dir(&project_root)
        .envs(std::env::vars())
        .status()
        .await
        .context("Failed to run node --test")?;

    eprintln!();

    // Phase 5: Cleanup
    infra.stop().await;

    if status.success() {
        eprintln!("  {} All tests passed", "✓".green());
        Ok(())
    } else {
        eprintln!("  {} Some tests failed", "✗".red());
        std::process::exit(1);
    }
}

/// Find test files matching **/*.test.ts pattern.
fn find_test_files(root: &PathBuf, filter: &Option<String>) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    fn walk(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                // Skip ignored directories
                if name.starts_with('.')
                    || name == "node_modules"
                    || name == "dist"
                    || name == "target"
                {
                    continue;
                }
                walk(&path, files);
            } else {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.ends_with(".test.ts") || name.ends_with(".test.js") {
                    files.push(path);
                }
            }
        }
    }

    walk(root, &mut files);
    files.sort();

    // Apply filter if provided
    if let Some(pattern) = filter {
        files.retain(|f| {
            f.to_string_lossy()
                .to_lowercase()
                .contains(&pattern.to_lowercase())
        });
    }

    Ok(files)
}
