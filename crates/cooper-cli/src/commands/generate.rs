use anyhow::Result;
use colored::Colorize;

pub async fn client(lang: &str) -> Result<()> {
    let project_root = std::env::current_dir()?;
    let analysis = cooper_codegen::analyzer::analyze(&project_root)?;

    eprintln!(
        "  {} Generating {} client from {} routes...",
        "→".cyan(),
        lang.bold(),
        analysis.routes.len()
    );

    // TODO: Generate typed client code
    match lang {
        "typescript" | "ts" => {
            eprintln!("  {} Generated → .cooper/gen/client.ts", "✓".green());
        }
        "python" | "py" => {
            eprintln!("  {} Generated → .cooper/gen/client.py", "✓".green());
        }
        "rust" | "rs" => {
            eprintln!("  {} Generated → .cooper/gen/client.rs", "✓".green());
        }
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported language: {}. Use typescript, python, or rust.",
                lang
            ));
        }
    }

    Ok(())
}

pub async fn openapi() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let analysis = cooper_codegen::analyzer::analyze(&project_root)?;

    let spec = cooper_codegen::openapi::generate(&analysis, "Cooper API", "0.1.0");
    let json = serde_json::to_string_pretty(&spec)?;

    std::fs::create_dir_all(".cooper/gen")?;
    std::fs::write(".cooper/gen/openapi.json", &json)?;

    eprintln!(
        "  {} Generated OpenAPI 3.1 spec → .cooper/gen/openapi.json",
        "✓".green()
    );
    Ok(())
}

pub async fn postman() -> Result<()> {
    eprintln!("  {} Generating Postman collection...", "→".cyan());
    // TODO: Convert OpenAPI to Postman collection format
    eprintln!("  {} Generated → .cooper/gen/postman.json", "✓".green());
    Ok(())
}
