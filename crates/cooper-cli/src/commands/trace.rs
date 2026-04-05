use anyhow::Result;

pub async fn run(env: &str) -> Result<()> {
    eprintln!("Opening trace explorer for env '{}'...", env);
    // TODO: Open browser to trace explorer
    Ok(())
}
