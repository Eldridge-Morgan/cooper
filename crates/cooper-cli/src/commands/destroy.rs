use anyhow::Result;
use colored::Colorize;

pub async fn run(env: &str) -> Result<()> {
    eprintln!(
        "  {} Destroying environment '{}'...",
        "⚠".red(),
        env.bold()
    );
    // TODO: Tear down cloud resources
    eprintln!("  {} Environment destroyed", "✓".green());
    Ok(())
}
