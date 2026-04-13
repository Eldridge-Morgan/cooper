use anyhow::Result;
use colored::Colorize;
use cooper_deploy::{CloudProvider, ServiceType};
use dialoguer::{Confirm, Select};

pub async fn run(
    env: &str,
    cloud: &str,
    service: &str,
    dry_run: bool,
    auto_destroy_after: Option<String>,
    _app: Option<String>,
) -> Result<()> {
    let provider = CloudProvider::from_str(cloud)?;
    let service_type = ServiceType::from_str(service)?;

    let project_root = std::env::current_dir()?;

    // Read project name from directory name
    let project_name = project_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "app".to_string());

    // Step 1: Analyze project
    eprintln!(
        "  {} Analyzing project...",
        "\u{2192}".cyan()
    );
    let analysis = cooper_codegen::analyzer::analyze(&project_root)?;

    eprintln!(
        "  {} Found {} routes, {} databases, {} topics, {} queues",
        "\u{2713}".green(),
        analysis.routes.len(),
        analysis.databases.len(),
        analysis.topics.len(),
        analysis.queues.len()
    );

    // Step 2: Generate Terraform configuration
    eprintln!(
        "\n  {} Generating Terraform configuration ({} / {})...",
        "\u{2192}".cyan(),
        cloud.bold(),
        service_type.display_name().bold()
    );

    let tf_config = cooper_deploy::terraform::generate(
        &provider,
        &service_type,
        &analysis,
        env,
        &project_name,
    )?;

    // Step 3: Write to local directory
    let tf_dir = format!(".cooper/terraform/{env}");
    tf_config.write_to_disk(&tf_dir)?;

    eprintln!(
        "  {} Generated at {}/",
        "\u{2713}".green(),
        tf_dir.cyan()
    );

    // Step 4: Show preview
    eprintln!();
    eprintln!("{}", tf_config.format_preview());

    // Dry-run: stop here
    if dry_run {
        eprintln!(
            "  {} Dry run \u{2014} Terraform files written to {} but not applied.",
            "\u{2139}".blue(),
            tf_dir.cyan()
        );
        eprintln!("  You can inspect and edit the files, then run without --dry-run to deploy.");
        return Ok(());
    }

    // Step 5: Interactive menu
    let actions = vec![
        "Apply (proceed with deployment)",
        "Edit files (open in $EDITOR)",
        "Show full Terraform config",
        "Cancel",
    ];

    let action = Select::new()
        .with_prompt("  What would you like to do?")
        .items(&actions)
        .default(0)
        .interact()?;

    match action {
        0 => {} // Proceed to apply
        1 => {
            open_editor(&tf_dir)?;
            let proceed = Confirm::new()
                .with_prompt("  Files edited. Proceed with deployment?")
                .default(true)
                .interact()?;
            if !proceed {
                eprintln!("  {} Cancelled", "\u{2717}".red());
                return Ok(());
            }
        }
        2 => {
            show_full_config(&tf_dir)?;
            eprintln!();
            let proceed = Confirm::new()
                .with_prompt("  Proceed with deployment?")
                .default(false)
                .interact()?;
            if !proceed {
                eprintln!("  {} Cancelled", "\u{2717}".red());
                return Ok(());
            }
        }
        3 => {
            eprintln!("  {} Cancelled", "\u{2717}".red());
            return Ok(());
        }
        _ => unreachable!(),
    }

    // Step 6: Verify Terraform is installed before proceeding
    cooper_deploy::terraform::executor::check_terraform()?;

    // Step 7: Collect cloud credentials
    eprintln!(
        "\n  {} Checking cloud credentials...",
        "\u{2192}".cyan()
    );
    let credentials = cooper_deploy::credentials::collect(&provider).await?;

    // Step 7: Execute Terraform workflow
    eprintln!(
        "\n  {} Deploying infrastructure...\n",
        "\u{2192}".cyan()
    );

    let result = cooper_deploy::terraform::apply(
        &tf_dir,
        &credentials,
        env,
        &project_name,
    )
    .await?;

    // Step 8: Display results
    eprintln!(
        "\n  {} Deployment complete!",
        "\u{2713}".green()
    );

    if let Some(url) = &result.url {
        eprintln!("  {} {}", "\u{1f310}", url.cyan().underline());
    }

    eprintln!();
    for resource in &result.resources {
        let status_color = match resource.status.as_str() {
            "running" | "available" | "active" | "serving" => resource.status.green().to_string(),
            "creating" => resource.status.yellow().to_string(),
            _ => resource.status.dimmed().to_string(),
        };
        eprintln!(
            "  {} {} \u{2014} {}",
            resource.resource_type.bold(),
            resource.name.dimmed(),
            status_color,
        );
        if let Some(info) = &resource.connection_info {
            eprintln!("    {}", info.dimmed());
        }
    }

    eprintln!();
    eprintln!("  Terraform files: {}/", tf_dir);
    eprintln!("  Terraform state: {}/terraform.tfstate", tf_dir);
    eprintln!("  Deploy state:    .cooper/state/{}/deploy.json", env);

    if let Some(ttl) = auto_destroy_after {
        eprintln!(
            "\n  {} Environment will auto-destroy after {}",
            "\u{23f1}".yellow(),
            ttl
        );
    }

    eprintln!();
    Ok(())
}

/// Open the Terraform directory in the user's editor.
fn open_editor(tf_dir: &str) -> Result<()> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    let main_tf = format!("{tf_dir}/main.tf");
    eprintln!(
        "  Opening {} in {}...",
        main_tf.cyan(),
        editor.bold()
    );

    let status = std::process::Command::new(&editor)
        .arg(&main_tf)
        .status()?;

    if !status.success() {
        eprintln!(
            "  {} Editor exited with non-zero status",
            "!".yellow()
        );
    }

    Ok(())
}

/// Display the full generated Terraform config.
fn show_full_config(tf_dir: &str) -> Result<()> {
    for filename in &["provider.tf", "main.tf", "variables.tf", "outputs.tf"] {
        let path = format!("{tf_dir}/{filename}");
        if let Ok(content) = std::fs::read_to_string(&path) {
            eprintln!("\n  {} {}:", "\u{2500}".dimmed(), filename.bold());
            for line in content.lines() {
                eprintln!("    {}", line.dimmed());
            }
        }
    }
    Ok(())
}
