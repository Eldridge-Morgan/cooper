use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Stdio;

/// Tail logs from a deployed environment.
///
/// Reads deploy state to determine the cloud provider, then shells
/// out to the appropriate CLI to stream logs in real time.
pub async fn run(env: &str, service: Option<&str>) -> Result<()> {
    let state = cooper_deploy::state::load_state(env)?;

    let state = match state {
        Some(s) => s,
        None => {
            // No deployed state — try local logs
            return tail_local_logs(env, service).await;
        }
    };

    eprintln!(
        "  {} Tailing logs for {} ({})",
        "→".cyan(),
        env.bold(),
        state.provider.bold()
    );

    match state.provider.as_str() {
        "aws" => tail_aws_logs(env, service, &state).await,
        "gcp" => tail_gcp_logs(env, service, &state).await,
        "azure" => tail_azure_logs(env, service, &state).await,
        "fly" => tail_fly_logs(env, service, &state).await,
        _ => Err(anyhow::anyhow!("Unknown provider: {}", state.provider)),
    }
}

async fn tail_local_logs(env: &str, _service: Option<&str>) -> Result<()> {
    eprintln!(
        "  {} No deployment found for '{}'. Showing local logs.",
        "ℹ".blue(),
        env
    );

    // Tail the local .cooper/data/postgres/postgres.log if it exists
    let pg_log = ".cooper/data/postgres/postgres.log";
    if std::path::Path::new(pg_log).exists() {
        eprintln!("  {} Postgres log:", "─".dimmed());
        let content = std::fs::read_to_string(pg_log)?;
        for line in content.lines().rev().take(20).collect::<Vec<_>>().into_iter().rev() {
            eprintln!("    {}", line.dimmed());
        }
    }

    eprintln!();
    eprintln!(
        "  {} Use {} to see server output",
        "ℹ".blue(),
        "cooper run".bold()
    );
    Ok(())
}

async fn tail_aws_logs(
    env: &str,
    service: Option<&str>,
    state: &cooper_deploy::DeployResult,
) -> Result<()> {
    // Find the ECS service name from state
    let project_name = &state.env;
    let log_group = format!("/ecs/cooper-{project_name}-{env}");

    let _svc_filter = service
        .map(|s| format!("--filter-pattern \"{}\"", s))
        .unwrap_or_default();

    eprintln!("  {} Streaming from CloudWatch: {}", "→".cyan(), log_group.dimmed());
    eprintln!();

    // Use `aws logs tail` for real-time streaming
    let mut cmd = tokio::process::Command::new("aws");
    cmd.args(["logs", "tail", &log_group, "--follow", "--format", "short"]);

    if let Some(svc) = service {
        cmd.args(["--filter-pattern", svc]);
    }

    let mut child = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("AWS CLI not found. Install: https://aws.amazon.com/cli/")?;

    child.wait().await?;
    Ok(())
}

async fn tail_gcp_logs(
    env: &str,
    _service: Option<&str>,
    state: &cooper_deploy::DeployResult,
) -> Result<()> {
    let app_resource = state
        .resources
        .iter()
        .find(|r| r.resource_type == "Cloud Run")
        .map(|r| r.name.clone())
        .unwrap_or_else(|| format!("cooper-{}-{env}-app", state.env));

    eprintln!(
        "  {} Streaming from Cloud Logging: {}",
        "→".cyan(),
        app_resource.dimmed()
    );
    eprintln!();

    let mut cmd = tokio::process::Command::new("gcloud");
    cmd.args([
        "logging",
        "tail",
        &format!("resource.type=cloud_run_revision AND resource.labels.service_name={app_resource}"),
        "--format",
        "value(textPayload)",
    ]);

    let mut child = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("gcloud CLI not found. Install: https://cloud.google.com/sdk")?;

    child.wait().await?;
    Ok(())
}

async fn tail_azure_logs(
    env: &str,
    _service: Option<&str>,
    state: &cooper_deploy::DeployResult,
) -> Result<()> {
    let rg = format!("cooper-{}-{env}-rg", state.env);
    let app_name = state
        .resources
        .iter()
        .find(|r| r.resource_type == "Container App")
        .map(|r| r.name.clone())
        .unwrap_or_else(|| format!("cooper-{}-{env}-app", state.env));

    eprintln!(
        "  {} Streaming from Container Apps: {}",
        "→".cyan(),
        app_name.dimmed()
    );
    eprintln!();

    let mut child = tokio::process::Command::new("az")
        .args([
            "containerapp",
            "logs",
            "show",
            "--resource-group",
            &rg,
            "--name",
            &app_name,
            "--follow",
            "--type",
            "console",
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("Azure CLI not found. Install: https://learn.microsoft.com/en-us/cli/azure/install-azure-cli")?;

    child.wait().await?;
    Ok(())
}

async fn tail_fly_logs(
    env: &str,
    _service: Option<&str>,
    state: &cooper_deploy::DeployResult,
) -> Result<()> {
    let app_name = state
        .resources
        .iter()
        .find(|r| r.resource_type == "Fly Machine")
        .map(|r| r.name.clone())
        .unwrap_or_else(|| format!("cooper-{}-{env}-app", state.env));

    eprintln!(
        "  {} Streaming from Fly.io: {}",
        "→".cyan(),
        app_name.dimmed()
    );
    eprintln!();

    let mut child = tokio::process::Command::new("flyctl")
        .args(["logs", "--app", &app_name])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("flyctl not found. Install: https://fly.io/docs/hands-on/install-flyctl/")?;

    child.wait().await?;
    Ok(())
}
