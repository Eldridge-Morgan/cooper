use anyhow::Result;

pub async fn ls() -> Result<()> {
    eprintln!("Environments:");
    eprintln!("  local  (running)");
    // TODO: List from state store
    Ok(())
}

pub async fn url(env: &str) -> Result<()> {
    // TODO: Look up from state store
    println!("https://{env}.cooperdev.io");
    Ok(())
}
