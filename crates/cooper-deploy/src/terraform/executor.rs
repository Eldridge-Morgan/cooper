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

/// Run `terraform refresh` to sync state with actual cloud resources.
pub async fn terraform_refresh(tf_dir: &str, credentials: &Credentials) -> Result<()> {
    eprintln!("  {} terraform refresh", "  $".dimmed());
    let output = Command::new("terraform")
        .args(["refresh", "-no-color", "-input=false"])
        .current_dir(tf_dir)
        .envs(credentials.env_vars())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run terraform refresh")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("  {} terraform refresh warning: {}", "!".yellow(), stderr.lines().next().unwrap_or(""));
    } else {
        eprintln!("  {} terraform refresh complete", "  ok".green());
    }

    Ok(())
}

/// Run `terraform import` to bring an existing cloud resource into state.
async fn terraform_import(tf_dir: &str, credentials: &Credentials, address: &str, id: &str) -> Result<()> {
    eprintln!("  {} terraform import {} {}", "  $".dimmed(), address, id);
    let output = Command::new("terraform")
        .args(["import", "-no-color", "-input=false", address, id])
        .current_dir(tf_dir)
        .envs(credentials.env_vars())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
        .context("Failed to run terraform import")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("terraform import failed for {}: {}", address, stderr));
    }

    eprintln!("  {} imported {}", "  ok".green(), address);
    Ok(())
}

/// Parse "already exists" errors from terraform apply output.
/// Returns vec of (resource_address, cloud_id) tuples.
fn parse_already_exists_errors(output: &str) -> Vec<(String, String)> {
    let mut results = Vec::new();
    let lines: Vec<&str> = output.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        let is_already_exists = line.contains("AlreadyExists")
            || line.contains("already exists")
            || line.contains("EntityAlreadyExists")
            || line.contains("ResourceInUseException")
            || line.contains("BucketAlreadyOwnedByYou")
            || line.contains("RepositoryAlreadyExistsException");

        if !is_already_exists {
            continue;
        }

        // Look for the resource address in nearby "with" line
        let mut address = None;
        let mut cloud_id = None;

        // Search backwards and forwards for "with <resource_type>.<name>"
        let search_range = i.saturating_sub(5)..=(i + 5).min(lines.len() - 1);
        for j in search_range {
            let l = lines[j].trim();
            if l.starts_with("with ") {
                // "with aws_elasticache_subnet_group.cache,"
                let addr = l.trim_start_matches("with ").trim_end_matches(',').trim();
                address = Some(addr.to_string());
            }
        }

        // Extract cloud ID from parentheses in the error line
        // e.g. "creating ElastiCache Subnet Group (cooper-testing-prod-cache-subnet-group)"
        if let Some(start) = line.find('(') {
            if let Some(end) = line[start..].find(')') {
                let id = &line[start + 1..start + end];
                if !id.contains(' ') || id.contains("arn:") {
                    cloud_id = Some(id.to_string());
                }
            }
        }

        if let (Some(addr), Some(id)) = (address, cloud_id) {
            results.push((addr, id));
        }
    }

    results
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
/// Retries on dependency errors (e.g. AWS ENI not yet released after ECS task stops).
pub async fn terraform_destroy(tf_dir: &str, credentials: &Credentials) -> Result<()> {
    let max_retries = 3;

    for attempt in 0..=max_retries {
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

        if output.status.success() {
            eprintln!("  {} terraform destroy complete", "  ok".green());
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let is_dependency_error = stderr.contains("DependencyViolation")
            || stderr.contains("has a dependent object")
            || stderr.contains("has an associated interface")
            || stderr.contains("network interface is in use")
            || stderr.contains("resource still in use");

        if is_dependency_error && attempt < max_retries {
            let wait_secs = 30 * (attempt + 1);
            eprintln!(
                "  {} Resource dependency conflict (ENI still attached). Retrying in {}s... ({}/{})",
                "!".yellow(),
                wait_secs,
                attempt + 1,
                max_retries
            );
            tokio::time::sleep(std::time::Duration::from_secs(wait_secs as u64)).await;
        } else {
            return Err(anyhow::anyhow!("terraform destroy failed:\n{}", stderr));
        }
    }

    unreachable!()
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

    // Refresh state to sync with actual cloud resources (handles cancelled deploys)
    let tf_state_path = format!("{tf_dir}/terraform.tfstate");
    if std::path::Path::new(&tf_state_path).exists() {
        terraform_refresh(tf_dir, credentials).await?;
    }

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

    // Apply — with auto-import retry on "already exists" errors
    let apply_result = terraform_apply(tf_dir, credentials).await;
    if let Err(ref e) = apply_result {
        let err_msg = e.to_string();
        let orphans = parse_already_exists_errors(&err_msg);

        if !orphans.is_empty() {
            eprintln!(
                "\n  {} Found {} orphaned resource(s) from a previous cancelled deploy. Importing...",
                "!".yellow(),
                orphans.len()
            );

            for (addr, id) in &orphans {
                if let Err(import_err) = terraform_import(tf_dir, credentials, addr, id).await {
                    eprintln!("  {} Failed to import {}: {}", "!".yellow(), addr, import_err);
                }
            }

            // Re-plan and apply after importing
            eprintln!("\n  {} Re-running plan + apply after import...\n", "→".cyan());
            terraform_plan(tf_dir, credentials).await?;
            terraform_apply(tf_dir, credentials).await?;
        } else {
            apply_result?;
        }
    }

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
