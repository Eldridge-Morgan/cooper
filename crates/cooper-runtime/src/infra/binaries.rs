//! Auto-download infrastructure binaries to ~/.cooper/bin/
//!
//! On first `cooper run`, if nats-server / valkey / postgres are not on PATH,
//! we download pre-built binaries from GitHub releases and cache them locally.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Where managed binaries live.
fn bin_dir() -> PathBuf {
    dirs_home().join(".cooper").join("bin")
}

pub fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

/// Platform identifier for download URLs.
fn platform() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86_64") {
        "amd64"
    } else {
        "unknown"
    };

    (os, arch)
}

/// Binary metadata — name, version, and how to download it.
struct BinarySpec {
    name: &'static str,
    version: &'static str,
    url_template: fn(os: &str, arch: &str, version: &str) -> String,
    archive_type: ArchiveType,
    /// Path inside the archive to the binary (relative).
    inner_path: fn(os: &str, arch: &str, version: &str) -> String,
}

enum ArchiveType {
    TarGz,
    Zip,
    /// Direct binary download, no archive
    #[allow(dead_code)]
    Raw,
}

/// All managed binaries.
fn specs() -> Vec<BinarySpec> {
    vec![
        BinarySpec {
            name: "nats-server",
            version: "2.10.24",
            url_template: |os, arch, ver| {
                // NATS uses amd64/arm64 and darwin/linux
                format!(
                    "https://github.com/nats-io/nats-server/releases/download/v{ver}/nats-server-v{ver}-{os}-{arch}.zip"
                )
            },
            archive_type: ArchiveType::Zip,
            inner_path: |os, arch, ver| {
                format!("nats-server-v{ver}-{os}-{arch}/nats-server")
            },
        },
        BinarySpec {
            name: "valkey-server",
            version: "8.1.1",
            url_template: |os, arch, ver| {
                // Valkey provides tarballs on GitHub
                let arch_label = if arch == "arm64" { "arm64" } else { "x86_64" };
                let os_label = if os == "darwin" { "macos" } else { "linux" };
                format!(
                    "https://github.com/valkey-io/valkey/releases/download/{ver}/valkey-{ver}-{os_label}-{arch_label}.tar.gz"
                )
            },
            archive_type: ArchiveType::TarGz,
            inner_path: |os, arch, ver| {
                let arch_label = if arch == "arm64" { "arm64" } else { "x86_64" };
                let os_label = if os == "darwin" { "macos" } else { "linux" };
                format!("valkey-{ver}-{os_label}-{arch_label}/bin/valkey-server")
            },
        },
    ]
}

/// Find a binary: first check PATH, then ~/.cooper/bin/, then auto-download.
pub async fn resolve_binary(name: &str) -> Result<String> {
    // 1. Already on PATH?
    if let Ok(path) = which::which(name) {
        return Ok(path.to_string_lossy().to_string());
    }

    // 2. Already in ~/.cooper/bin/?
    let managed_path = bin_dir().join(name);
    if managed_path.exists() {
        return Ok(managed_path.to_string_lossy().to_string());
    }

    // 3. Auto-download
    let spec = specs()
        .into_iter()
        .find(|s| s.name == name)
        .ok_or_else(|| anyhow::anyhow!(
            "{name} not found on PATH and no auto-download available. Install it manually."
        ))?;

    tracing::info!("Downloading {name} v{}...", spec.version);
    download_binary(&spec).await?;

    if managed_path.exists() {
        Ok(managed_path.to_string_lossy().to_string())
    } else {
        Err(anyhow::anyhow!("Download completed but {name} not found at {}", managed_path.display()))
    }
}

/// Resolve a Postgres binary (pg_ctl, initdb, psql, createdb).
/// Downloads the full Postgres distribution on first use from theseus-rs/postgresql-binaries.
pub async fn resolve_postgres(binary_name: &str) -> Result<String> {
    // 1. Already on PATH?
    if let Ok(path) = which::which(binary_name) {
        return Ok(path.to_string_lossy().to_string());
    }

    // 2. Already in ~/.cooper/pg/bin/?
    let pg_dir = dirs_home().join(".cooper").join("pg");
    let managed = pg_dir.join("bin").join(binary_name);
    if managed.exists() {
        return Ok(managed.to_string_lossy().to_string());
    }

    // 3. Auto-download the full Postgres distribution
    tracing::info!("Downloading PostgreSQL (first run only)...");
    download_postgres(&pg_dir).await?;

    if managed.exists() {
        Ok(managed.to_string_lossy().to_string())
    } else {
        Err(anyhow::anyhow!(
            "PostgreSQL download completed but {} not found at {}",
            binary_name,
            managed.display()
        ))
    }
}

/// Download pre-built PostgreSQL binaries from theseus-rs/postgresql-binaries.
/// Extracts the full distribution (bin/, lib/, share/) to ~/.cooper/pg/.
async fn download_postgres(pg_dir: &Path) -> Result<()> {
    let version = "17.4.0";

    let triple = if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "x86_64-unknown-linux-gnu"
    } else {
        return Err(anyhow::anyhow!("Unsupported platform for PostgreSQL auto-download"));
    };

    let url = format!(
        "https://github.com/theseus-rs/postgresql-binaries/releases/download/{version}/postgresql-{version}-{triple}.tar.gz"
    );

    tracing::info!("  Fetching {url}");
    let response = reqwest::get(&url)
        .await
        .context(format!("Failed to download PostgreSQL from {url}"))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "PostgreSQL download failed: HTTP {} for {url}",
            response.status()
        ));
    }

    let bytes = response.bytes().await?;

    // Extract to a temp dir first, then move
    let tmp_dir = tempfile::tempdir().context("Failed to create temp dir")?;
    let archive_path = tmp_dir.path().join("pg.tar.gz");
    std::fs::write(&archive_path, &bytes)?;

    let file = std::fs::File::open(&archive_path)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(gz);
    archive.unpack(tmp_dir.path())?;

    // Find the extracted directory (postgresql-{version}-{triple}/)
    let prefix = format!("postgresql-{version}-{triple}");
    let extracted = tmp_dir.path().join(&prefix);

    if !extracted.exists() {
        // Try to find any postgresql-* directory
        let mut found = None;
        for entry in std::fs::read_dir(tmp_dir.path())? {
            let entry = entry?;
            if entry.file_name().to_string_lossy().starts_with("postgresql-") {
                found = Some(entry.path());
                break;
            }
        }
        let extracted = found.ok_or_else(|| {
            anyhow::anyhow!("Could not find PostgreSQL directory in archive")
        })?;
        std::fs::create_dir_all(pg_dir)?;
        // Move bin/, lib/, share/ into pg_dir
        for subdir in &["bin", "lib", "share"] {
            let src = extracted.join(subdir);
            let dst = pg_dir.join(subdir);
            if src.exists() {
                if dst.exists() {
                    std::fs::remove_dir_all(&dst)?;
                }
                rename_or_copy_dir(&src, &dst)?;
            }
        }
    } else {
        std::fs::create_dir_all(pg_dir)?;
        for subdir in &["bin", "lib", "share"] {
            let src = extracted.join(subdir);
            let dst = pg_dir.join(subdir);
            if src.exists() {
                if dst.exists() {
                    std::fs::remove_dir_all(&dst)?;
                }
                rename_or_copy_dir(&src, &dst)?;
            }
        }
    }

    // Set executable permissions on all binaries
    if let Ok(entries) = std::fs::read_dir(pg_dir.join("bin")) {
        for entry in entries.flatten() {
            let _ = set_executable(&entry.path());
        }
    }

    tracing::info!("  Installed PostgreSQL {} to {}", version, pg_dir.display());
    Ok(())
}

/// Move a directory, falling back to recursive copy if rename fails (cross-device).
fn rename_or_copy_dir(src: &Path, dst: &Path) -> Result<()> {
    if std::fs::rename(src, dst).is_ok() {
        return Ok(());
    }
    // Fallback: recursive copy
    copy_dir_recursive(src, dst)
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

async fn download_binary(spec: &BinarySpec) -> Result<()> {
    let (os, arch) = platform();
    let url = (spec.url_template)(os, arch, spec.version);
    let inner_path = (spec.inner_path)(os, arch, spec.version);

    let dir = bin_dir();
    std::fs::create_dir_all(&dir)?;

    let tmp_dir = tempfile::tempdir().context("Failed to create temp dir")?;

    // Download
    tracing::info!("  Fetching {url}");
    let response = reqwest::get(&url).await.context(format!("Failed to download {url}"))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Download failed: HTTP {} for {url}",
            response.status()
        ));
    }

    let bytes = response.bytes().await?;
    let archive_path = tmp_dir.path().join("archive");
    std::fs::write(&archive_path, &bytes)?;

    // Extract
    match spec.archive_type {
        ArchiveType::TarGz => {
            let file = std::fs::File::open(&archive_path)?;
            let gz = flate2::read::GzDecoder::new(file);
            let mut archive = tar::Archive::new(gz);

            for entry in archive.entries()? {
                let mut entry = entry?;
                let path = entry.path()?.to_path_buf();
                let path_str = path.to_string_lossy().to_string();

                if path_str == inner_path || path_str.ends_with(&format!("/{}", spec.name)) {
                    let dest = dir.join(spec.name);
                    entry.unpack(&dest)?;
                    set_executable(&dest)?;
                    tracing::info!("  Installed {} to {}", spec.name, dest.display());
                    return Ok(());
                }
            }

            // If exact match not found, try extracting all and finding it
            let file = std::fs::File::open(&archive_path)?;
            let gz = flate2::read::GzDecoder::new(file);
            let mut archive = tar::Archive::new(gz);
            archive.unpack(tmp_dir.path())?;

            let expected = tmp_dir.path().join(&inner_path);
            if expected.exists() {
                let dest = dir.join(spec.name);
                std::fs::copy(&expected, &dest)?;
                set_executable(&dest)?;
                tracing::info!("  Installed {} to {}", spec.name, dest.display());
                return Ok(());
            }

            Err(anyhow::anyhow!(
                "Could not find {} in archive (looked for {})",
                spec.name,
                inner_path,
            ))
        }

        ArchiveType::Zip => {
            let file = std::fs::File::open(&archive_path)?;
            let mut archive = zip::ZipArchive::new(file)?;

            for i in 0..archive.len() {
                let mut entry = archive.by_index(i)?;
                let name = entry.name().to_string();

                if name == inner_path || name.ends_with(&format!("/{}", spec.name)) {
                    let dest = dir.join(spec.name);
                    let mut out = std::fs::File::create(&dest)?;
                    std::io::copy(&mut entry, &mut out)?;
                    set_executable(&dest)?;
                    tracing::info!("  Installed {} to {}", spec.name, dest.display());
                    return Ok(());
                }
            }

            Err(anyhow::anyhow!(
                "Could not find {} in zip archive (looked for {})",
                spec.name,
                inner_path,
            ))
        }

        ArchiveType::Raw => {
            let dest = dir.join(spec.name);
            std::fs::write(&dest, &bytes)?;
            set_executable(&dest)?;
            tracing::info!("  Installed {} to {}", spec.name, dest.display());
            Ok(())
        }
    }
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}
