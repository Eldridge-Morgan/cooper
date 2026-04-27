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

    // Step 2: Copy SDK source files flat into dist/ so bridge.ts imports resolve
    eprintln!("  {} Packaging bridge...", "→".cyan());
    let sdk_src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sdk/src");
    if sdk_src.exists() {
        for entry in std::fs::read_dir(&sdk_src)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                std::fs::copy(&path, output_dir.join(entry.file_name()))?;
            }
        }
    }

    // Step 3: Copy SDK into sdk/ subdirectory as well
    if sdk_src.exists() {
        let sdk_out = output_dir.join("sdk");
        std::fs::create_dir_all(&sdk_out)?;
        copy_dir_recursive(&sdk_src, &sdk_out)?;
    }

    // Step 3b: Bundle cooper SDK into node_modules/cooper so services can import 'cooper/api'
    for pkg_name in &["@eldridge-morgan/cooper", "cooper"] {
        let nm_pkg = project_root.join("node_modules").join(pkg_name);
        if nm_pkg.exists() && !nm_pkg.is_symlink() {
            let dest = output_dir.join("node_modules").join(pkg_name);
            std::fs::create_dir_all(&dest)?;
            copy_dir_recursive(&nm_pkg, &dest)?;
            // Also create cooper/ alias pointing to @eldridge-morgan/cooper
            let alias = output_dir.join("node_modules/cooper");
            if !alias.exists() {
                let alias_dest = output_dir.join("node_modules/@eldridge-morgan/cooper");
                std::fs::create_dir_all(alias.parent().unwrap())?;
                copy_dir_recursive(&alias_dest, &alias)?;
            }
            break;
        }
    }

    // Step 3c: Overlay local SDK dist/ on top of node_modules copies
    // so local fixes always take precedence over the published npm package
    let sdk_dist = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sdk/dist");
    if sdk_dist.exists() {
        for pkg_name in &["@eldridge-morgan/cooper", "cooper"] {
            let dest_dist = output_dir.join("node_modules").join(pkg_name).join("dist");
            if dest_dist.exists() {
                copy_dir_recursive(&sdk_dist, &dest_dist)?;
            }
        }
    }

    // Step 4a: Copy services directory (needed at runtime for JS worker imports)
    let services_dir = project_root.join("services");
    if services_dir.exists() {
        let svc_out = output_dir.join("services");
        std::fs::create_dir_all(&svc_out)?;
        copy_dir_recursive(&services_dir, &svc_out)?;
    }

    // Step 4b: Copy config file if present
    for config_name in &["cooper.config.ts", "cooper.config.js"] {
        let config_path = project_root.join(config_name);
        if config_path.exists() {
            std::fs::copy(&config_path, output_dir.join(config_name))?;
            break;
        }
    }

    // Step 4c: Copy shared directory if present
    let shared_dir = project_root.join("shared");
    if shared_dir.exists() {
        let shared_out = output_dir.join("shared");
        std::fs::create_dir_all(&shared_out)?;
        copy_dir_recursive(&shared_dir, &shared_out)?;
    }

    // Step 4d: Copy package.json, stripping cooper SDK (already bundled in dist/node_modules/)
    let pkg_json = project_root.join("package.json");
    if pkg_json.exists() {
        let mut obj: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&pkg_json)?)?;
        // Strip the cooper SDK — it's already bundled in dist/node_modules/
        for key in &["dependencies", "devDependencies"] {
            if let Some(deps) = obj.get_mut(*key).and_then(|d| d.as_object_mut()) {
                deps.retain(|name, _| !name.contains("cooper"));
            }
        }
        // Merge in the SDK's own runtime deps so bun installs them in the container
        let sdk_deps = [("pg", "^8.13.0"), ("mysql2", "^3.11.0"), ("ioredis", "^5.4.0"), ("nats", "^2.29.0")];
        let deps = obj
            .get_mut("dependencies")
            .and_then(|d| d.as_object_mut());
        if let Some(deps) = deps {
            for (name, version) in &sdk_deps {
                deps.entry(*name).or_insert_with(|| serde_json::json!(*version));
            }
        }
        std::fs::write(output_dir.join("package.json"), serde_json::to_string_pretty(&obj)?)?;
    }

    // Step 5: Copy migrations (renumbered from 4)
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

    // Step: Copy the cooper binary into dist/ for Docker builds
    let cooper_bin = std::env::current_exe()?;
    std::fs::copy(&cooper_bin, output_dir.join("cooper"))?;

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
            .args(["--outfile", output.to_str().unwrap()])
            .args(["--target", "bun"])
            .args(["--external", "cooper", "--external", "cooper/*"])
            .args(["--external", "pg", "--external", "mysql2", "--external", "ioredis", "--external", "nats"])
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

fn generate_dockerfile(_dist_dir: &str) -> String {
    r#"# syntax=docker/dockerfile:1
FROM oven/bun:1

WORKDIR /app

# Install postgresql-client for running migrations
RUN apt-get update -qq && apt-get install -y --no-install-recommends postgresql-client && rm -rf /var/lib/apt/lists/*

# Load package manifest — cache layer only invalidates when package.json changes
COPY package.json ./

# BuildKit cache mount: bun's global cache persists across builds,
# packages are only re-downloaded when package.json/lockfile diff
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --production

# Copy the rest of the build
COPY . .

# Install cooper binary
RUN mv cooper /usr/local/bin/cooper && chmod +x /usr/local/bin/cooper

EXPOSE 4000
ENV COOPER_ENV=production
ENV NODE_ENV=production

CMD ["./entrypoint.sh"]
"#
    .to_string()
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

# Start the production server
exec cooper serve
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
