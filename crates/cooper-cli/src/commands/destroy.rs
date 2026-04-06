use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

pub async fn run(env: &str) -> Result<()> {
    let state = cooper_deploy::state::load_state(env)?;

    match state {
        Some(state) => {
            eprintln!(
                "  {} Environment '{}' ({})",
                "⚠".red(),
                env.bold(),
                state.provider.bold()
            );
            eprintln!(
                "    {} resources will be destroyed:",
                state.resources.len()
            );
            for r in &state.resources {
                eprintln!("    - {} ({})", r.name, r.resource_type);
            }
            eprintln!();

            let proceed = Confirm::new()
                .with_prompt("  Destroy all resources?")
                .default(false)
                .interact()?;

            if !proceed {
                eprintln!("  {} Cancelled", "✗".red());
                return Ok(());
            }

            let provider = cooper_deploy::CloudProvider::from_str(&state.provider)?;
            let project_name = &state.env;

            eprintln!(
                "\n  {} Destroying...\n",
                "→".cyan()
            );

            cooper_deploy::provisioner::destroy(&provider, env, project_name).await?;

            // Clean up local state
            let state_dir = format!(".cooper/state/{env}");
            let _ = std::fs::remove_dir_all(&state_dir);

            eprintln!(
                "  {} Environment '{}' destroyed",
                "✓".green(),
                env
            );
        }
        None => {
            return Err(anyhow::anyhow!(
                "Environment '{}' not found. Run `cooper env ls` to see environments.",
                env
            ));
        }
    }

    Ok(())
}
