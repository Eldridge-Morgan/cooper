use anyhow::Result;
use colored::Colorize;

pub async fn set(name: &str, env: &str) -> Result<()> {
    eprintln!("  Enter value for '{}' ({}): ", name.bold(), env);
    // TODO: Read from stdin securely, store in vault
    eprintln!("  {} Secret '{}' set for env '{}'", "✓".green(), name, env);
    Ok(())
}

pub async fn ls(env: &str) -> Result<()> {
    eprintln!("  Secrets for env '{}':", env.bold());
    // TODO: List from vault
    eprintln!("  (none)");
    Ok(())
}

pub async fn rm(name: &str, env: &str) -> Result<()> {
    // TODO: Remove from vault
    eprintln!(
        "  {} Secret '{}' removed from env '{}'",
        "✓".green(),
        name,
        env
    );
    Ok(())
}
