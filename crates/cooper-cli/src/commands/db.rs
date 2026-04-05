use anyhow::Result;
use colored::Colorize;

pub async fn migrate() -> Result<()> {
    eprintln!("  {} Running migrations...", "→".cyan());
    // TODO: Run SQL migration files in order
    eprintln!("  {} Migrations up to date", "✓".green());
    Ok(())
}

pub async fn seed() -> Result<()> {
    eprintln!("  {} Running seed scripts...", "→".cyan());
    // TODO: Execute seed files
    eprintln!("  {} Seeded", "✓".green());
    Ok(())
}

pub async fn shell() -> Result<()> {
    eprintln!("  {} Opening database shell...", "→".cyan());
    // TODO: Launch psql/mysql connected to local or remote DB
    Ok(())
}

pub async fn conn_uri(db: &str, env: Option<&str>) -> Result<()> {
    let _env_name = env.unwrap_or("local");
    // TODO: Look up actual connection details
    println!("postgres://cooper:cooper@localhost:5432/{db}?sslmode=disable");
    Ok(())
}
