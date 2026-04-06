use anyhow::Result;
use colored::Colorize;

pub async fn ls() -> Result<()> {
    // Check for expired environments and destroy them
    match cooper_deploy::scheduler::check_expired_environments().await {
        Ok(destroyed) => {
            for env in &destroyed {
                eprintln!(
                    "  {} Auto-destroyed expired environment: {}",
                    "⏱".yellow(),
                    env
                );
            }
        }
        Err(_) => {}
    }

    eprintln!("  {}", "Environments:".bold());
    eprintln!("  {} local  {}", "●".green(), "(always available)".dimmed());

    let envs = cooper_deploy::state::list_environments()?;
    if envs.is_empty() {
        eprintln!();
        eprintln!(
            "  {} Deploy with: {}",
            "ℹ".blue(),
            "cooper deploy --env staging --cloud aws".dimmed()
        );
    } else {
        for env_name in &envs {
            if let Ok(Some(state)) = cooper_deploy::state::load_state(env_name) {
                let url = state.url.as_deref().unwrap_or("-");
                let resource_count = state.resources.len();

                // Check for auto-destroy countdown
                let ttl_info = cooper_deploy::scheduler::time_remaining(env_name)
                    .map(|(remaining, _)| format!(" [{}]", remaining))
                    .unwrap_or_default();

                eprintln!(
                    "  {} {}  {} ({} resources) {}{}",
                    "●".cyan(),
                    env_name,
                    state.provider.dimmed(),
                    resource_count,
                    url.dimmed(),
                    ttl_info.yellow()
                );
            } else {
                eprintln!("  {} {}", "●".yellow(), env_name);
            }
        }
    }
    eprintln!();
    Ok(())
}

pub async fn url(env: &str) -> Result<()> {
    if env == "local" {
        println!("http://localhost:4000");
        return Ok(());
    }

    match cooper_deploy::state::load_state(env)? {
        Some(state) => {
            if let Some(url) = &state.url {
                println!("{url}");
            } else {
                eprintln!("No URL available for environment '{env}'");
            }
        }
        None => {
            return Err(anyhow::anyhow!(
                "Environment '{}' not found. Run `cooper env ls` to see available environments.",
                env
            ));
        }
    }

    Ok(())
}
