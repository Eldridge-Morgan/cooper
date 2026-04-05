use anyhow::Result;
use colored::Colorize;
use cooper_deploy::{CloudProvider, diff};

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

    eprintln!("  Proceed? [y/N] ");
    // TODO: Read user input
    // For now, just show the plan
    eprintln!("  {} Deployment would proceed here", "→".cyan());

    if let Some(ttl) = auto_destroy_after {
        eprintln!(
            "  {} Environment will auto-destroy after {}",
            "⏱".yellow(),
            ttl
        );
    }

    Ok(())
}
