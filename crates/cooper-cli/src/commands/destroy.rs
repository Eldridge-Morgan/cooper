use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;
use std::path::Path;

pub async fn run(env: &str) -> Result<()> {
    eprintln!(
        "  {} Destroying environment '{}'...",
        "\u{26a0}".red(),
        env.bold()
    );

    let tf_dir = format!(".cooper/terraform/{env}");
    let tf_state = format!("{tf_dir}/terraform.tfstate");

    // Check if Terraform state exists
    if Path::new(&tf_state).exists() {
        eprintln!(
            "  Found Terraform state at {}",
            tf_state.dimmed()
        );

        let proceed = Confirm::new()
            .with_prompt(format!(
                "  This will destroy ALL resources in '{}'. Are you sure?",
                env
            ))
            .default(false)
            .interact()?;

        if !proceed {
            eprintln!("  {} Cancelled", "\u{2717}".red());
            return Ok(());
        }

        // Check terraform is installed
        cooper_deploy::terraform::executor::check_terraform()?;

        // Detect provider from state
        let state_path = format!(".cooper/state/{env}/deploy.json");
        let provider_name = if Path::new(&state_path).exists() {
            let content = std::fs::read_to_string(&state_path)?;
            let state: serde_json::Value = serde_json::from_str(&content)?;
            state
                .get("provider")
                .and_then(|v| v.as_str())
                .unwrap_or("aws")
                .to_string()
        } else {
            "aws".to_string()
        };

        let provider = cooper_deploy::CloudProvider::from_str(&provider_name)?;

        // Collect credentials
        eprintln!(
            "  {} Checking cloud credentials...",
            "\u{2192}".cyan()
        );
        let credentials = cooper_deploy::credentials::collect(&provider).await?;

        // Run terraform destroy
        eprintln!(
            "\n  {} Running terraform destroy...\n",
            "\u{2192}".cyan()
        );
        cooper_deploy::terraform::destroy(&tf_dir, &credentials).await?;

        // Clean up state
        let state_dir = format!(".cooper/state/{env}");
        if Path::new(&state_dir).exists() {
            std::fs::remove_dir_all(&state_dir)?;
        }

        eprintln!(
            "\n  {} Environment '{}' destroyed",
            "\u{2713}".green(),
            env
        );
    } else {
        // No Terraform state — check if there's a Cooper state from the old provisioner
        let state_path = format!(".cooper/state/{env}/deploy.json");
        if Path::new(&state_path).exists() {
            eprintln!(
                "  {} This environment was deployed without Terraform.",
                "!".yellow()
            );
            eprintln!("  Manual cleanup may be required for cloud resources.");

            let proceed = Confirm::new()
                .with_prompt("  Remove local state anyway?")
                .default(false)
                .interact()?;

            if proceed {
                std::fs::remove_dir_all(format!(".cooper/state/{env}"))?;
                eprintln!("  {} Local state removed", "\u{2713}".green());
            }
        } else {
            eprintln!(
                "  {} No deployment found for environment '{}'",
                "\u{2717}".red(),
                env
            );
        }
    }

    Ok(())
}
