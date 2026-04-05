use anyhow::Result;
use colored::Colorize;

pub async fn run() -> Result<()> {
    eprintln!(
        "  {} MCP server started — connect from Cursor, Claude, or Copilot",
        "🔌".to_string().cyan()
    );
    // TODO: Start MCP server over stdio
    Ok(())
}
