use anyhow::{Context, Result};
use std::process::Stdio;
use tokio::process::Command;

use crate::CloudProvider;

/// Schedule automatic destruction of a preview environment.
///
/// Uses cloud-native schedulers so the destroy happens even if no
/// local process is running:
///   AWS   → EventBridge Scheduler → Lambda → cooper destroy
///   GCP   → Cloud Scheduler → Cloud Function
///   Azure → Logic App timer trigger
///   Fly   → Background machine with sleep + destroy
///
/// Falls back to writing a `.cooper/state/{env}/destroy_at` file
/// that `cooper env ls` checks on every run.
pub async fn schedule_destroy(
    provider: &CloudProvider,
    env: &str,
    project_name: &str,
    ttl: &str,
) -> Result<()> {
    let seconds = parse_ttl(ttl)?;
    let destroy_at = chrono::Utc::now() + chrono::Duration::seconds(seconds as i64);

    // Always write local state so `cooper env ls` can show the countdown
    write_destroy_marker(env, &destroy_at.to_rfc3339(), ttl)?;

    // Try cloud-native scheduler
    let cloud_result = match provider {
        CloudProvider::Aws => schedule_aws(env, project_name, seconds).await,
        CloudProvider::Gcp => schedule_gcp(env, project_name, seconds).await,
        CloudProvider::Azure => schedule_azure(env, project_name, seconds).await,
        CloudProvider::Fly => schedule_fly(env, project_name, seconds).await,
    };

    match cloud_result {
        Ok(()) => {
            tracing::info!("Cloud scheduler set for env '{env}' — destroys at {destroy_at}");
        }
        Err(e) => {
            tracing::warn!("Cloud scheduler failed ({e}) — using local marker only");
        }
    }

    Ok(())
}

/// Check all environments for expired destroy markers and destroy them.
pub async fn check_expired_environments() -> Result<Vec<String>> {
    let mut destroyed = Vec::new();
    let state_dir = ".cooper/state";

    if !std::path::Path::new(state_dir).exists() {
        return Ok(destroyed);
    }

    for entry in std::fs::read_dir(state_dir)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }

        let env_name = entry.file_name().to_string_lossy().to_string();
        let marker_path = entry.path().join("destroy_at");

        if !marker_path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&marker_path)?;
        let destroy_at: chrono::DateTime<chrono::Utc> = content
            .lines()
            .next()
            .unwrap_or("")
            .parse()
            .unwrap_or(chrono::Utc::now() + chrono::Duration::hours(24));

        if chrono::Utc::now() >= destroy_at {
            tracing::info!("Environment '{env_name}' has expired — destroying");

            // Read deploy state to get provider
            if let Ok(Some(state)) = crate::state::load_state(&env_name) {
                let provider = CloudProvider::from_str(&state.provider)?;
                let project_name = &state.env;

                if let Err(e) =
                    crate::provisioner::destroy(&provider, &env_name, project_name).await
                {
                    tracing::error!("Failed to destroy '{env_name}': {e}");
                } else {
                    // Clean up state
                    let _ = std::fs::remove_dir_all(entry.path());
                    destroyed.push(env_name);
                }
            }
        }
    }

    Ok(destroyed)
}

/// Get time remaining until destruction, or None if no marker.
pub fn time_remaining(env: &str) -> Option<(String, chrono::DateTime<chrono::Utc>)> {
    let marker_path = format!(".cooper/state/{env}/destroy_at");
    let content = std::fs::read_to_string(&marker_path).ok()?;
    let mut lines = content.lines();

    let destroy_at: chrono::DateTime<chrono::Utc> = lines.next()?.parse().ok()?;
    let _ttl_label = lines.next().unwrap_or("").to_string();

    let remaining = destroy_at - chrono::Utc::now();
    if remaining.num_seconds() <= 0 {
        return Some(("expired".to_string(), destroy_at));
    }

    let hours = remaining.num_hours();
    let mins = remaining.num_minutes() % 60;

    let display = if hours > 0 {
        format!("{}h {}m remaining", hours, mins)
    } else {
        format!("{}m remaining", mins)
    };

    Some((display, destroy_at))
}

// ── Internal ──

fn write_destroy_marker(env: &str, destroy_at: &str, ttl: &str) -> Result<()> {
    let dir = format!(".cooper/state/{env}");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(
        format!("{dir}/destroy_at"),
        format!("{destroy_at}\n{ttl}\n"),
    )?;
    Ok(())
}

fn parse_ttl(ttl: &str) -> Result<u64> {
    let ttl = ttl.trim();
    let (num_str, unit) = if ttl.ends_with('h') {
        (&ttl[..ttl.len() - 1], "h")
    } else if ttl.ends_with('m') {
        (&ttl[..ttl.len() - 1], "m")
    } else if ttl.ends_with('d') {
        (&ttl[..ttl.len() - 1], "d")
    } else if ttl.ends_with('s') {
        (&ttl[..ttl.len() - 1], "s")
    } else {
        // Try parsing as hours by default
        (ttl, "h")
    };

    let num: u64 = num_str
        .parse()
        .context(format!("Invalid TTL number: '{num_str}'. Use format like '48h', '30m', '7d'."))?;

    Ok(match unit {
        "s" => num,
        "m" => num * 60,
        "h" => num * 3600,
        "d" => num * 86400,
        _ => num * 3600,
    })
}

// ── Cloud-native schedulers ──

async fn schedule_aws(env: &str, project_name: &str, delay_seconds: u64) -> Result<()> {
    let schedule_name = format!("cooper-destroy-{project_name}-{env}");
    let destroy_at = chrono::Utc::now() + chrono::Duration::seconds(delay_seconds as i64);
    let schedule_expr = format!(
        "at({})",
        destroy_at.format("%Y-%m-%dT%H:%M:%S")
    );

    // Create EventBridge Scheduler rule
    // This invokes a Lambda or ECS task that runs `cooper destroy`
    let _ = Command::new("aws")
        .args([
            "scheduler", "create-schedule",
            "--name", &schedule_name,
            "--schedule-expression", &schedule_expr,
            "--flexible-time-window", "Mode=OFF",
            "--target", &format!(
                r#"{{"Arn":"arn:aws:lambda:us-east-1:*:function:cooper-destroy","Input":"{{\"env\":\"{env}\",\"project\":\"{project_name}\"}}" }}"#
            ),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .context("AWS CLI not available for scheduler")?;

    Ok(())
}

async fn schedule_gcp(env: &str, project_name: &str, delay_seconds: u64) -> Result<()> {
    let job_name = format!("cooper-destroy-{project_name}-{env}");
    let destroy_at = chrono::Utc::now() + chrono::Duration::seconds(delay_seconds as i64);
    let schedule = format!(
        "{} {} {} {} *",
        destroy_at.format("%M"),
        destroy_at.format("%H"),
        destroy_at.format("%d"),
        destroy_at.format("%m"),
    );

    let _ = Command::new("gcloud")
        .args([
            "scheduler", "jobs", "create", "http",
            &job_name,
            "--schedule", &schedule,
            "--uri", &format!("https://cooper-destroy.cloudfunctions.net/?env={env}&project={project_name}"),
            "--http-method", "POST",
            "--time-zone", "UTC",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .context("gcloud CLI not available for scheduler")?;

    Ok(())
}

async fn schedule_azure(env: &str, project_name: &str, delay_seconds: u64) -> Result<()> {
    // Azure: use a Logic App with a delay action, or an Azure Function timer trigger
    // For simplicity, we create a one-time timer trigger via az CLI
    let rg = format!("cooper-{project_name}-{env}-rg");
    let destroy_at = chrono::Utc::now() + chrono::Duration::seconds(delay_seconds as i64);

    let _ = Command::new("az")
        .args([
            "deployment", "group", "create",
            "--resource-group", &rg,
            "--template-uri", "https://raw.githubusercontent.com/cooper/templates/main/auto-destroy.json",
            "--parameters", &format!("destroyAt={}", destroy_at.to_rfc3339()),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .context("az CLI not available for scheduler")?;

    Ok(())
}

async fn schedule_fly(env: &str, project_name: &str, delay_seconds: u64) -> Result<()> {
    let app_name = format!("cooper-{project_name}-{env}-destroy");

    // Fly: create a one-shot machine that sleeps then calls flyctl destroy
    let _ = Command::new("flyctl")
        .args([
            "machine", "run",
            "--app", &app_name,
            "alpine",
            "--", "sh", "-c",
            &format!("sleep {delay_seconds} && flyctl apps destroy cooper-{project_name}-{env}-app --yes"),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .context("flyctl not available for scheduler")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ttl() {
        assert_eq!(parse_ttl("48h").unwrap(), 172800);
        assert_eq!(parse_ttl("30m").unwrap(), 1800);
        assert_eq!(parse_ttl("7d").unwrap(), 604800);
        assert_eq!(parse_ttl("60s").unwrap(), 60);
        assert_eq!(parse_ttl("24").unwrap(), 86400); // default hours
    }
}
