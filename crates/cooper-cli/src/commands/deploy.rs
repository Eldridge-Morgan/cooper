use anyhow::Result;
use colored::Colorize;
use cooper_deploy::{CloudProvider, diff};
use dialoguer::Confirm;

pub async fn run(
    env: &str,
    cloud: &str,
    dry_run: bool,
    auto_destroy_after: Option<String>,
    _app: Option<String>,
) -> Result<()> {
    let provider = CloudProvider::from_str(cloud)?;
    let project_root = std::env::current_dir()?;
    let analysis = cooper_codegen::analyzer::analyze(&project_root)?;

    // Read project name from cooper.config.ts or directory name
    let project_name = project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "app".to_string());

    eprintln!(
        "  {} Planning deployment to {} ({})",
        "→".cyan(),
        env.bold(),
        cloud.bold()
    );

    let plan = cooper_deploy::cloud::plan_deployment(&provider, &analysis, env)?;

    eprintln!();
    eprintln!("{}", diff::format_plan(&plan));

    if dry_run {
        eprintln!("  {} Dry run — no changes made", "ℹ".blue());
        return Ok(());
    }

    // Confirm deployment
    let proceed = Confirm::new()
        .with_prompt("  Proceed?")
        .default(false)
        .interact()?;

    if !proceed {
        eprintln!("  {} Cancelled", "✗".red());
        return Ok(());
    }

    // Execute the deployment
    eprintln!(
        "\n  {} Provisioning {} resources...\n",
        "→".cyan(),
        cloud.bold()
    );

    let result = cooper_deploy::provisioner::provision(
        &provider,
        &plan,
        &analysis,
        env,
        &project_name,
    )
    .await?;

    eprintln!();
    eprintln!(
        "  {} Deployment complete!",
        "✓".green()
    );

    if let Some(url) = &result.url {
        eprintln!(
            "  {} {}",
            "🌐",
            url.cyan().underline()
        );
    }

    eprintln!();
    for resource in &result.resources {
        let status_color = match resource.status.as_str() {
            "running" | "available" | "active" | "serving" => resource.status.green().to_string(),
            "creating" => resource.status.yellow().to_string(),
            _ => resource.status.dimmed().to_string(),
        };
        eprintln!(
            "  {} {} — {}",
            resource.resource_type.bold(),
            resource.name.dimmed(),
            status_color,
        );
        if let Some(info) = &resource.connection_info {
            eprintln!("    {}", info.dimmed());
        }
    }

    if let Some(ttl) = auto_destroy_after {
        eprintln!(
            "\n  {} Environment will auto-destroy after {}",
            "⏱".yellow(),
            ttl
        );
        // TODO: Schedule destruction via cloud scheduler
    }

    eprintln!();
    Ok(())
}
