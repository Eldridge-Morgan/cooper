use anyhow::{Context, Result};
use colored::Colorize;

/// Open the trace explorer for a deployed or local environment.
///
/// - Local: opens the Cooper dashboard trace view at localhost:9400/traces
/// - Deployed: opens the cloud provider's trace viewer (X-Ray, Cloud Trace, etc.)
///   or the configured observability provider (Datadog, Grafana, Axiom).
pub async fn run(env: &str) -> Result<()> {
    let state = cooper_deploy::state::load_state(env)?;

    match state {
        Some(state) => open_cloud_traces(env, &state).await,
        None => open_local_traces(env).await,
    }
}

async fn open_local_traces(_env: &str) -> Result<()> {
    let url = "http://localhost:9400/traces";

    eprintln!(
        "  {} Opening local trace explorer: {}",
        "→".cyan(),
        url.cyan().underline()
    );

    open_browser(url)?;
    Ok(())
}

async fn open_cloud_traces(env: &str, state: &cooper_deploy::DeployResult) -> Result<()> {
    let url = match state.provider.as_str() {
        "aws" => {
            let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".to_string());
            format!(
                "https://{region}.console.aws.amazon.com/cloudwatch/home?region={region}#xray:traces"
            )
        }
        "gcp" => {
            let project = std::env::var("GOOGLE_CLOUD_PROJECT").unwrap_or_default();
            format!(
                "https://console.cloud.google.com/traces/list?project={project}"
            )
        }
        "azure" => {
            "https://portal.azure.com/#view/Microsoft_Azure_Monitoring/AzureMonitoringBrowseBlade/~/overview".to_string()
        }
        "fly" => {
            let app_name = state
                .resources
                .iter()
                .find(|r| r.resource_type == "Fly Machine")
                .map(|r| r.name.clone())
                .unwrap_or_default();
            format!("https://fly.io/apps/{app_name}/monitoring")
        }
        _ => {
            return Err(anyhow::anyhow!("Unknown provider: {}", state.provider));
        }
    };

    // Check if there's a custom observability config
    let config_path = "cooper.config.ts";
    if std::path::Path::new(config_path).exists() {
        let config_content = std::fs::read_to_string(config_path)?;
        // Quick check for known providers
        if config_content.contains("datadog") {
            let dd_url = "https://app.datadoghq.com/apm/traces";
            eprintln!(
                "  {} Datadog configured — opening: {}",
                "→".cyan(),
                dd_url.cyan().underline()
            );
            open_browser(dd_url)?;
            return Ok(());
        }
        if config_content.contains("grafana") {
            eprintln!(
                "  {} Grafana configured — check your Grafana Tempo dashboard",
                "→".cyan(),
            );
        }
    }

    eprintln!(
        "  {} Opening trace explorer for {} ({}): {}",
        "→".cyan(),
        env.bold(),
        state.provider.bold(),
        url.cyan().underline()
    );

    open_browser(&url)?;
    Ok(())
}

fn open_browser(url: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .context("Failed to open browser")?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("Failed to open browser")?;
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", url])
            .spawn()
            .context("Failed to open browser")?;
    }
    Ok(())
}
