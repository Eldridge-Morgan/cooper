use anyhow::Result;
use colored::Colorize;

pub async fn run() -> Result<()> {
    eprintln!(
        "  {} Serving docs at {}",
        "📖",
        "http://localhost:9401".cyan().underline()
    );
    // TODO: Start docs server
    Ok(())
}
