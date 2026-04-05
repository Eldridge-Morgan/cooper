use anyhow::Result;
use colored::Colorize;
use std::path::{Path, PathBuf};
use std::process::Stdio;

pub async fn run(output: &str) -> Result<()> {
    eprintln!("  {} Building for production...", "→".cyan());

    let project_root = std::env::current_dir()?;
    let analysis = cooper_codegen::analyzer::analyze(&project_root)?;
    let output_dir = PathBuf::from(output);

    eprintln!(
        "  {} Analyzed {} routes, {} databases, {} topics, {} queues",
        "✓".green(),
        analysis.routes.len(),
        analysis.databases.len(),
        analysis.topics.len(),
        analysis.queues.len(),
    );

    std::fs::create_dir_all(&output_dir)?;

    // Step 1: Bundle TypeScript using Bun or esbuild
    eprintln!("  {} Bundling TypeScript...", "→".cyan());
    let bundle_path = output_dir.join("bundle.js");
    bundle_typescript(&project_root, &bundle_path).await?;
    eprintln!("  {} Bundle created ({:.1} KB)", "✓".green(),
        std::fs::metadata(&bundle_path).map(|m| m.len() as f64 / 1024.0).unwrap_or(0.0));

    // Step 2: Copy the bridge
    eprintln!("  {} Packaging bridge...", "→".cyan());
    let sdk_bridge = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sdk/src/bridge.ts");
    if sdk_bridge.exists() {
        std::fs::copy(&sdk_bridge, output_dir.join("bridge.ts"))?;
    }

    // Step 3: Copy SDK
    let sdk_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sdk/src");
    if sdk_src.exists() {
        let sdk_out = output_dir.join("sdk");
        std::fs::create_dir_all(&sdk_out)?;
        copy_dir_recursive(&sdk_src, &sdk_out)?;
    }

    // Step 4: Copy migrations
    let migrations_dir = project_root.join("migrations");
    if migrations_dir.exists() {
        let mig_out = output_dir.join("migrations");
        std::fs::create_dir_all(&mig_out)?;
        copy_dir_recursive(&migrations_dir, &mig_out)?;
    }

    // Step 5: Write the analysis manifest
    let manifest = serde_json::json!({
        "routes": analysis.routes,
        "databases": analysis.databases,
        "topics": analysis.topics,
        "crons": analysis.crons,
        "queues": analysis.queues,
        "pages": analysis.pages,
    });
    std::fs::write(
        output_dir.join("cooper-manifest.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    // Step 6: Bundle SSR pages if any
    if !analysis.pages.is_empty() {
        eprintln!("  {} Bundling {} SSR pages...", "→".cyan(), analysis.pages.len());
        let pages_out = output_dir.join("pages");
        std::fs::create_dir_all(&pages_out)?;
        let pages_dir = project_root.join("pages");
        if pages_dir.exists() {
            copy_dir_recursive(&pages_dir, &pages_out)?;
        }
    }

    // Step 7: Bundle islands for client-side hydration
    let islands_dir = project_root.join("islands");
    if islands_dir.exists() {
        eprintln!("  {} Bundling islands...", "→".cyan());
        let islands_out = output_dir.join("islands");
        std::fs::create_dir_all(&islands_out)?;
        bundle_islands(&islands_dir, &islands_out).await?;
    }

    // Step 8: Generate Dockerfile
    eprintln!("  {} Generating Dockerfile...", "→".cyan());
    let dockerfile = generate_dockerfile(output);
    std::fs::write(output_dir.join("Dockerfile"), &dockerfile)?;

    // Step 9: Write entrypoint script
    let entrypoint = generate_entrypoint();
    std::fs::write(output_dir.join("entrypoint.sh"), &entrypoint)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            output_dir.join("entrypoint.sh"),
            std::fs::Permissions::from_mode(0o755),
        )?;
    }

    // Stats
    let total_size = dir_size(&output_dir)?;
    eprintln!();
    eprintln!(
        "  {} Build complete → {} ({:.1} MB)",
        "✓".green(),
        output.bold(),
        total_size as f64 / (1024.0 * 1024.0)
    );
    eprintln!();
    eprintln!("  Run locally:  cd {} && ./entrypoint.sh", output);
    eprintln!("  Docker:       docker build -t myapp {} && docker run -p 4000:4000 myapp", output);
    eprintln!("  Deploy:       cooper deploy --env prod --cloud aws");
    eprintln!();

    Ok(())
}

/// Bundle all TypeScript service files into a single JS bundle.
async fn bundle_typescript(project_root: &Path, output: &Path) -> Result<()> {
    // Collect all .ts entry points
    let mut entrypoints = Vec::new();
    let services_dir = project_root.join("services");
    if services_dir.exists() {
        collect_ts_files(&services_dir, &mut entrypoints)?;
    }

    if entrypoints.is_empty() {
        std::fs::write(output, "// No services to bundle\n")?;
        return Ok(());
    }

    // Try bun first, then esbuild, then just copy
    if which::which("bun").is_ok() {
        let entrypoint_args: Vec<String> = entrypoints
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        let status = tokio::process::Command::new("bun")
            .arg("build")
            .args(&entrypoint_args)
            .args(["--outdir", output.parent().unwrap().to_str().unwrap()])
            .args(["--target", "bun"])
            .args(["--external", "cooper", "--external", "cooper/*"])
            .args(["--external", "pg", "--external", "mysql2", "--external", "ioredis"])
            .current_dir(project_root)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .status()
            .await?;

        if !status.success() {
            // Fallback: just concatenate the files
            concat_ts_files(&entrypoints, output)?;
        }
    } else {
        concat_ts_files(&entrypoints, output)?;
    }

    Ok(())
}

/// Bundle island components for client-side delivery.
async fn bundle_islands(islands_dir: &Path, output_dir: &Path) -> Result<()> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(islands_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.to_string_lossy().contains(".island.") {
            entries.push(path);
        }
    }

    if entries.is_empty() {
        return Ok(());
    }

    if which::which("bun").is_ok() {
        let entry_args: Vec<String> = entries.iter().map(|p| p.to_string_lossy().to_string()).collect();

        let _ = tokio::process::Command::new("bun")
            .arg("build")
            .args(&entry_args)
            .args(["--outdir", output_dir.to_str().unwrap()])
            .args(["--target", "browser"])
            .args(["--splitting", "--format", "esm"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;
    } else {
        // Copy source files as fallback
        for entry in &entries {
            let name = entry.file_name().unwrap();
            std::fs::copy(entry, output_dir.join(name))?;
        }
    }

    Ok(())
}

fn collect_ts_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_ts_files(&path, out)?;
        } else if matches!(path.extension().and_then(|e| e.to_str()), Some("ts" | "tsx")) {
            out.push(path);
        }
    }
    Ok(())
}

fn concat_ts_files(files: &[PathBuf], output: &Path) -> Result<()> {
    let mut content = String::new();
    for file in files {
        content.push_str(&format!("// --- {} ---\n", file.display()));
        content.push_str(&std::fs::read_to_string(file)?);
        content.push('\n');
    }
    std::fs::write(output, content)?;
    Ok(())
}

fn generate_dockerfile(dist_dir: &str) -> String {
    format!(
        r#"FROM oven/bun:1-alpine

WORKDIR /app

# Copy production bundle
COPY {dist_dir}/ .

# Install runtime dependencies
RUN bun install --production 2>/dev/null || true

EXPOSE 4000
ENV COOPER_ENV=production
ENV NODE_ENV=production

CMD ["./entrypoint.sh"]
"#
    )
}

fn generate_entrypoint() -> String {
    r#"#!/bin/sh
set -e

# Run migrations if present
if [ -d "./migrations" ]; then
    echo "[cooper] Running migrations..."
    for f in ./migrations/*.sql; do
        [ -f "$f" ] && psql "$COOPER_DB_MAIN_URL" -f "$f" 2>/dev/null || true
    done
fi

# Start the application
exec bun run bridge.ts
"#
    .to_string()
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn dir_size(dir: &Path) -> Result<u64> {
    let mut total = 0u64;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            total += dir_size(&path)?;
        } else {
            total += std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(total)
}
