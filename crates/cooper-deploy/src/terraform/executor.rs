use crate::{DeployResult, ProvisionedResource};
use crate::credentials::Credentials;
use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Stdio;
use tokio::process::Command;

/// Check if Terraform is installed and accessible.
pub fn check_terraform() -> Result<()> {
    which::which("terraform").context(
        "Terraform CLI not found. Install it: https://developer.hashicorp.com/terraform/install",
    )?;
    Ok(())
}

/// Run `terraform init` in the given directory.
pub async fn terraform_init(tf_dir: &str, credentials: &Credentials) -> Result<()> {
    eprintln!("  {} terraform init", "  $".dimmed());
    let output = Command::new("terraform")
        .args(["init", "-no-color", "-input=false"])
        .current_dir(tf_dir)
        .envs(credentials.env_vars())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run terraform init")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("terraform init failed:\n{}", stderr));
    }

    eprintln!("  {} terraform init complete", "  ok".green());
    Ok(())
}

/// Run `terraform plan` and return the plan output.
pub async fn terraform_plan(tf_dir: &str, credentials: &Credentials) -> Result<String> {
    eprintln!("  {} terraform plan", "  $".dimmed());
    let output = Command::new("terraform")
        .args(["plan", "-no-color", "-input=false", "-out=tfplan"])
        .current_dir(tf_dir)
        .envs(credentials.env_vars())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run terraform plan")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "terraform plan failed:\n{}\n{}",
            stdout,
            stderr
        ));
    }

    eprintln!("  {} terraform plan complete", "  ok".green());
    Ok(stdout)
}

/// Run `terraform validate` in the given directory.
pub async fn terraform_validate(tf_dir: &str, credentials: &Credentials) -> Result<String> {
    eprintln!("  {} terraform validate", "  $".dimmed());
    let output = Command::new("terraform")
        .args(["validate", "-no-color"])
        .current_dir(tf_dir)
        .envs(credentials.env_vars())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run terraform validate")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "terraform validate failed:\n{}\n{}",
            stdout,
            stderr
        ));
    }

    eprintln!("  {} terraform validate passed", "  ok".green());
    Ok(stdout)
}

/// Run `terraform apply` using the saved plan.
pub async fn terraform_apply(tf_dir: &str, credentials: &Credentials) -> Result<String> {
    eprintln!("  {} terraform apply", "  $".dimmed());
    let output = Command::new("terraform")
        .args(["apply", "-no-color", "-input=false", "-auto-approve", "tfplan"])
        .current_dir(tf_dir)
        .envs(credentials.env_vars())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run terraform apply")?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "terraform apply failed:\n{}\n{}",
            stdout,
            stderr
        ));
    }

    eprintln!("  {} terraform apply complete", "  ok".green());
    Ok(stdout)
}

/// Run `terraform output -json` and parse it.
pub async fn terraform_output(tf_dir: &str, credentials: &Credentials) -> Result<serde_json::Value> {
    let output = Command::new("terraform")
        .args(["output", "-json"])
        .current_dir(tf_dir)
        .envs(credentials.env_vars())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run terraform output")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_default();
    Ok(parsed)
}

/// Run `terraform destroy` in the given directory.
pub async fn terraform_destroy(tf_dir: &str, credentials: &Credentials) -> Result<()> {
    eprintln!("  {} terraform destroy", "  $".dimmed());
    let output = Command::new("terraform")
        .args(["destroy", "-no-color", "-input=false", "-auto-approve"])
        .current_dir(tf_dir)
        .envs(credentials.env_vars())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run terraform destroy")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("terraform destroy failed:\n{}", stderr));
    }

    eprintln!("  {} terraform destroy complete", "  ok".green());
    Ok(())
}

/// Execute full Terraform workflow: init → plan → apply → extract outputs.
/// Returns a DeployResult with resource info and connection strings.
pub async fn execute_terraform(
    tf_dir: &str,
    credentials: &Credentials,
    env: &str,
    project_name: &str,
) -> Result<DeployResult> {
    // Init
    terraform_init(tf_dir, credentials).await?;

    // Plan
    let plan_output = terraform_plan(tf_dir, credentials).await?;

    // Show plan summary
    let adds = plan_output.matches("will be created").count();
    let changes = plan_output.matches("will be updated").count();
    let destroys = plan_output.matches("will be destroyed").count();
    eprintln!(
        "\n  Plan: {} to add, {} to change, {} to destroy\n",
        format!("{adds}").green(),
        format!("{changes}").yellow(),
        format!("{destroys}").red(),
    );

    // Apply
    terraform_apply(tf_dir, credentials).await?;

    // Extract outputs
    let outputs = terraform_output(tf_dir, credentials).await?;

    // Build resources list from outputs
    let mut resources = Vec::new();
    let mut url = None;

    if let Some(obj) = outputs.as_object() {
        for (key, val) in obj {
            let value_str = val
                .get("value")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            // Try to detect the main URL
            if (key.contains("url") || key.contains("fqdn") || key.contains("endpoint"))
                && (value_str.starts_with("http") || value_str.contains('.'))
            {
                if url.is_none() {
                    url = Some(value_str.clone());
                }
            }

            let sensitive = val.get("sensitive").and_then(|v| v.as_bool()).unwrap_or(false);
            let display_value = if sensitive {
                "(sensitive)".to_string()
            } else {
                value_str.clone()
            };

            resources.push(ProvisionedResource {
                resource_type: "terraform_output".to_string(),
                name: key.clone(),
                status: "active".to_string(),
                connection_info: Some(display_value),
            });
        }
    }

    // Save state
    let state = serde_json::json!({
        "env": env,
        "project": project_name,
        "provider": credentials.provider_name(),
        "tf_dir": tf_dir,
        "outputs": outputs,
        "deployed_at": chrono::Utc::now().to_rfc3339(),
    });

    let state_dir = format!(".cooper/state/{env}");
    std::fs::create_dir_all(&state_dir)?;
    std::fs::write(
        format!("{state_dir}/deploy.json"),
        serde_json::to_string_pretty(&state)?,
    )?;

    Ok(DeployResult {
        env: env.to_string(),
        provider: credentials.provider_name().to_string(),
        url,
        resources,
    })
}
