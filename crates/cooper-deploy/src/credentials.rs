use crate::CloudProvider;
use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::{Input, Password};
use std::collections::HashMap;
use std::path::Path;

/// Cloud credentials needed for Terraform execution.
#[derive(Debug, Clone)]
pub struct Credentials {
    provider: CloudProvider,
    env_map: HashMap<String, String>,
}

impl Credentials {
    /// Return environment variables for passing to Terraform.
    pub fn env_vars(&self) -> &HashMap<String, String> {
        &self.env_map
    }

    /// Return the cloud provider name.
    pub fn provider_name(&self) -> &str {
        match self.provider {
            CloudProvider::Aws => "aws",
            CloudProvider::Gcp => "gcp",
            CloudProvider::Azure => "azure",
            CloudProvider::Fly => "fly",
        }
    }
}

/// Collect cloud credentials for the given provider.
/// Checks existing env vars / config files first, then prompts for missing ones.
pub async fn collect(provider: &CloudProvider) -> Result<Credentials> {
    match provider {
        CloudProvider::Aws => collect_aws().await,
        CloudProvider::Gcp => collect_gcp().await,
        CloudProvider::Azure => collect_azure().await,
        CloudProvider::Fly => collect_fly().await,
    }
}

async fn collect_aws() -> Result<Credentials> {
    let mut env_map = HashMap::new();

    // Check env vars first
    let has_key = std::env::var("AWS_ACCESS_KEY_ID").is_ok();
    let has_secret = std::env::var("AWS_SECRET_ACCESS_KEY").is_ok();
    let has_profile = std::env::var("AWS_PROFILE").is_ok();
    let has_aws_dir = Path::new(&dirs_home().join(".aws/credentials")).exists();

    if has_key && has_secret {
        eprintln!("  {} AWS credentials found (env vars)", "ok".green());
        env_map.insert(
            "AWS_ACCESS_KEY_ID".to_string(),
            std::env::var("AWS_ACCESS_KEY_ID").unwrap(),
        );
        env_map.insert(
            "AWS_SECRET_ACCESS_KEY".to_string(),
            std::env::var("AWS_SECRET_ACCESS_KEY").unwrap(),
        );
        if let Ok(token) = std::env::var("AWS_SESSION_TOKEN") {
            env_map.insert("AWS_SESSION_TOKEN".to_string(), token);
        }
    } else if has_profile || has_aws_dir {
        eprintln!("  {} AWS credentials found (CLI profile)", "ok".green());
        if let Ok(profile) = std::env::var("AWS_PROFILE") {
            env_map.insert("AWS_PROFILE".to_string(), profile);
        }
    } else {
        eprintln!(
            "  {} No AWS credentials found. Please provide them:",
            "!".yellow()
        );

        let access_key: String = Input::new()
            .with_prompt("  AWS Access Key ID")
            .interact_text()
            .context("Failed to read AWS Access Key ID")?;

        let secret_key: String = Password::new()
            .with_prompt("  AWS Secret Access Key")
            .interact()
            .context("Failed to read AWS Secret Access Key")?;

        env_map.insert("AWS_ACCESS_KEY_ID".to_string(), access_key);
        env_map.insert("AWS_SECRET_ACCESS_KEY".to_string(), secret_key);
    }

    // Region
    let region = std::env::var("AWS_REGION")
        .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
        .unwrap_or_else(|_| "us-east-1".to_string());
    env_map.insert("AWS_REGION".to_string(), region.clone());
    env_map.insert("AWS_DEFAULT_REGION".to_string(), region);

    Ok(Credentials {
        provider: CloudProvider::Aws,
        env_map,
    })
}

async fn collect_gcp() -> Result<Credentials> {
    let mut env_map = HashMap::new();

    // Check for Application Default Credentials
    let has_adc = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").is_ok();
    let has_gcloud = which::which("gcloud").is_ok();
    let adc_path = dirs_home().join(".config/gcloud/application_default_credentials.json");
    let has_default_adc = adc_path.exists();

    if has_adc {
        eprintln!(
            "  {} GCP credentials found (GOOGLE_APPLICATION_CREDENTIALS)",
            "ok".green()
        );
        env_map.insert(
            "GOOGLE_APPLICATION_CREDENTIALS".to_string(),
            std::env::var("GOOGLE_APPLICATION_CREDENTIALS").unwrap(),
        );
    } else if has_default_adc {
        eprintln!(
            "  {} GCP credentials found (application default credentials)",
            "ok".green()
        );
    } else if has_gcloud {
        eprintln!(
            "  {} No GCP credentials found. Run `gcloud auth application-default login` first.",
            "!".yellow()
        );
        return Err(anyhow::anyhow!(
            "GCP credentials not configured. Run: gcloud auth application-default login"
        ));
    } else {
        return Err(anyhow::anyhow!(
            "gcloud CLI not found. Install: https://cloud.google.com/sdk"
        ));
    }

    // Project ID
    let project_id = std::env::var("GOOGLE_CLOUD_PROJECT")
        .or_else(|_| std::env::var("GCP_PROJECT"))
        .or_else(|_| std::env::var("GCLOUD_PROJECT"));

    let project_id = match project_id {
        Ok(id) => id,
        Err(_) => {
            // Try gcloud
            if has_gcloud {
                let output = tokio::process::Command::new("gcloud")
                    .args(["config", "get-value", "project"])
                    .output()
                    .await?;
                let id = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if id.is_empty() || id == "(unset)" {
                    let id: String = Input::new()
                        .with_prompt("  GCP Project ID")
                        .interact_text()
                        .context("Failed to read GCP Project ID")?;
                    id
                } else {
                    id
                }
            } else {
                let id: String = Input::new()
                    .with_prompt("  GCP Project ID")
                    .interact_text()
                    .context("Failed to read GCP Project ID")?;
                id
            }
        }
    };

    eprintln!("  {} GCP project: {}", "ok".green(), project_id);
    env_map.insert("GOOGLE_CLOUD_PROJECT".to_string(), project_id.clone());
    env_map.insert("TF_VAR_gcp_project_id".to_string(), project_id);

    // Region
    let region = std::env::var("GOOGLE_CLOUD_REGION")
        .unwrap_or_else(|_| "us-central1".to_string());
    env_map.insert("GOOGLE_CLOUD_REGION".to_string(), region);

    Ok(Credentials {
        provider: CloudProvider::Gcp,
        env_map,
    })
}

async fn collect_azure() -> Result<Credentials> {
    let mut env_map = HashMap::new();

    // Check for Azure CLI login or service principal env vars
    let has_sp = std::env::var("ARM_CLIENT_ID").is_ok()
        && std::env::var("ARM_CLIENT_SECRET").is_ok()
        && std::env::var("ARM_TENANT_ID").is_ok()
        && std::env::var("ARM_SUBSCRIPTION_ID").is_ok();

    let has_az = which::which("az").is_ok();

    if has_sp {
        eprintln!(
            "  {} Azure credentials found (service principal env vars)",
            "ok".green()
        );
        for key in &[
            "ARM_CLIENT_ID",
            "ARM_CLIENT_SECRET",
            "ARM_TENANT_ID",
            "ARM_SUBSCRIPTION_ID",
        ] {
            if let Ok(val) = std::env::var(key) {
                env_map.insert(key.to_string(), val);
            }
        }
    } else if has_az {
        // Check if logged in
        let output = tokio::process::Command::new("az")
            .args(["account", "show", "--query", "id", "-o", "tsv"])
            .output()
            .await;

        match output {
            Ok(o) if o.status.success() => {
                let sub_id = String::from_utf8_lossy(&o.stdout).trim().to_string();
                eprintln!(
                    "  {} Azure credentials found (az CLI, subscription: {})",
                    "ok".green(),
                    &sub_id[..8.min(sub_id.len())]
                );
                env_map.insert("ARM_SUBSCRIPTION_ID".to_string(), sub_id);
            }
            _ => {
                eprintln!(
                    "  {} Not logged into Azure CLI. Run `az login` first.",
                    "!".yellow()
                );
                return Err(anyhow::anyhow!(
                    "Azure credentials not configured. Run: az login"
                ));
            }
        }
    } else {
        eprintln!(
            "  {} No Azure credentials found. Provide service principal details:",
            "!".yellow()
        );

        let client_id: String = Input::new()
            .with_prompt("  ARM_CLIENT_ID")
            .interact_text()?;
        let client_secret: String = Password::new()
            .with_prompt("  ARM_CLIENT_SECRET")
            .interact()?;
        let tenant_id: String = Input::new()
            .with_prompt("  ARM_TENANT_ID")
            .interact_text()?;
        let sub_id: String = Input::new()
            .with_prompt("  ARM_SUBSCRIPTION_ID")
            .interact_text()?;

        env_map.insert("ARM_CLIENT_ID".to_string(), client_id);
        env_map.insert("ARM_CLIENT_SECRET".to_string(), client_secret);
        env_map.insert("ARM_TENANT_ID".to_string(), tenant_id);
        env_map.insert("ARM_SUBSCRIPTION_ID".to_string(), sub_id);
    }

    // Location
    let location = std::env::var("AZURE_LOCATION").unwrap_or_else(|_| "eastus".to_string());
    env_map.insert("AZURE_LOCATION".to_string(), location);

    Ok(Credentials {
        provider: CloudProvider::Azure,
        env_map,
    })
}

async fn collect_fly() -> Result<Credentials> {
    let mut env_map = HashMap::new();

    let has_token = std::env::var("FLY_API_TOKEN").is_ok();
    let has_flyctl = which::which("flyctl").is_ok();

    if has_token {
        eprintln!("  {} Fly.io credentials found (FLY_API_TOKEN)", "ok".green());
        env_map.insert(
            "FLY_API_TOKEN".to_string(),
            std::env::var("FLY_API_TOKEN").unwrap(),
        );
    } else if has_flyctl {
        eprintln!(
            "  {} Using flyctl auth. Make sure you're logged in (`flyctl auth login`).",
            "ok".green()
        );
    } else {
        eprintln!(
            "  {} No Fly.io credentials found. Provide your API token:",
            "!".yellow()
        );
        let token: String = Password::new()
            .with_prompt("  FLY_API_TOKEN")
            .interact()
            .context("Failed to read Fly.io API token")?;
        env_map.insert("FLY_API_TOKEN".to_string(), token);
    }

    Ok(Credentials {
        provider: CloudProvider::Fly,
        env_map,
    })
}

fn dirs_home() -> std::path::PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
}
