use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

mod commands;

#[derive(Parser)]
#[command(name = "cooper", version, about = "The backend framework for TypeScript")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Cooper project
    New {
        /// Project name
        name: String,
        /// Template to use
        #[arg(long, default_value = "default")]
        template: String,
    },

    /// Start local dev server with hot reload
    Run {
        /// Run all apps in monorepo
        #[arg(long)]
        all: bool,
        /// Port to listen on
        #[arg(short, long, default_value = "4000")]
        port: u16,
    },

    /// Start production server (no embedded infra, no hot reload)
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "4000")]
        port: u16,
    },

    /// Run tests with embedded infrastructure
    Test {
        /// Run only tests matching this pattern
        #[arg(long)]
        filter: Option<String>,
        /// Stop on first failure
        #[arg(long)]
        fail_fast: bool,
    },

    /// Build for production
    Build {
        /// Output directory
        #[arg(short, long, default_value = "dist")]
        output: String,
    },

    /// Deploy to cloud
    Deploy {
        /// Target environment (e.g. prod, staging, dev)
        #[arg(long)]
        env: String,
        /// Cloud provider: aws, gcp, azure, fly
        #[arg(long)]
        cloud: String,
        /// Deployment model: server (container, default) or serverless (function)
        #[arg(long, default_value = "server")]
        service: String,
        /// Show Terraform config and cost estimate without deploying
        #[arg(long)]
        dry_run: bool,
        /// Auto-destroy after duration (e.g. "48h")
        #[arg(long)]
        auto_destroy_after: Option<String>,
        /// Deploy specific app in monorepo
        #[arg(long)]
        app: Option<String>,
    },

    /// Destroy an environment
    Destroy {
        /// Target environment
        #[arg(long)]
        env: String,
    },

    /// Database operations
    Db {
        #[command(subcommand)]
        command: DbCommands,
    },

    /// Secret management
    Secrets {
        #[command(subcommand)]
        command: SecretsCommands,
    },

    /// Generate clients, OpenAPI specs, Postman collections
    Gen {
        #[command(subcommand)]
        command: GenCommands,
    },

    /// Tail logs from a deployed environment
    Logs {
        #[arg(long)]
        env: String,
        #[arg(long)]
        service: Option<String>,
    },

    /// Open trace explorer
    Trace {
        #[arg(long)]
        env: String,
    },

    /// Run benchmarks
    Bench,

    /// Environment management
    Env {
        #[command(subcommand)]
        command: EnvCommands,
    },

    /// Serve API docs locally
    Docs,

    /// Start MCP server
    Mcp,
}

#[derive(Subcommand)]
enum DbCommands {
    /// Run pending migrations
    Migrate,
    /// Run seed scripts
    Seed,
    /// Open database shell
    Shell,
    /// Print connection URI
    ConnUri {
        /// Database name
        db: String,
        #[arg(long)]
        env: Option<String>,
    },
}

#[derive(Subcommand)]
enum SecretsCommands {
    /// Set a secret
    Set {
        name: String,
        #[arg(long)]
        env: String,
    },
    /// List secrets
    Ls {
        #[arg(long)]
        env: String,
    },
    /// Remove a secret
    Rm {
        name: String,
        #[arg(long)]
        env: String,
    },
}

#[derive(Subcommand)]
enum GenCommands {
    /// Generate a typed client
    Client {
        #[arg(long)]
        lang: String,
    },
    /// Generate OpenAPI 3.1 spec
    Openapi,
    /// Generate Postman collection
    Postman,
}

#[derive(Subcommand)]
enum EnvCommands {
    /// List all environments
    Ls,
    /// Get environment URL
    Url { env: String },
}

fn banner() {
    eprintln!(
        "{}",
        r#"
   ___
  / __\___   ___  _ __   ___ _ __
 / /  / _ \ / _ \| '_ \ / _ \ '__|
/ /__| (_) | (_) | |_) |  __/ |
\____/\___/ \___/| .__/ \___|_|
                  |_|
"#
        .cyan()
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env from the current directory if present (silently ignored if missing)
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cooper=info".into()),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::New { name, template } => {
            banner();
            commands::new_project::run(&name, &template).await
        }
        Commands::Run { all, port } => {
            banner();
            commands::run::run(all, port).await
        }
        Commands::Serve { port } => {
            commands::serve::run(port).await
        }
        Commands::Test { filter, fail_fast } => {
            banner();
            commands::test::run(filter, fail_fast).await
        }
        Commands::Build { output } => {
            banner();
            commands::build::run(&output).await
        }
        Commands::Deploy {
            env,
            cloud,
            service,
            dry_run,
            auto_destroy_after,
            app,
        } => {
            banner();
            commands::deploy::run(&env, &cloud, &service, dry_run, auto_destroy_after, app).await
        }
        Commands::Destroy { env } => {
            banner();
            commands::destroy::run(&env).await
        }
        Commands::Db { command } => match command {
            DbCommands::Migrate => commands::db::migrate().await,
            DbCommands::Seed => commands::db::seed().await,
            DbCommands::Shell => commands::db::shell().await,
            DbCommands::ConnUri { db, env } => commands::db::conn_uri(&db, env.as_deref()).await,
        },
        Commands::Secrets { command } => match command {
            SecretsCommands::Set { name, env } => commands::secrets::set(&name, &env).await,
            SecretsCommands::Ls { env } => commands::secrets::ls(&env).await,
            SecretsCommands::Rm { name, env } => commands::secrets::rm(&name, &env).await,
        },
        Commands::Gen { command } => match command {
            GenCommands::Client { lang } => commands::generate::client(&lang).await,
            GenCommands::Openapi => commands::generate::openapi().await,
            GenCommands::Postman => commands::generate::postman().await,
        },
        Commands::Logs { env, service } => commands::logs::run(&env, service.as_deref()).await,
        Commands::Trace { env } => commands::trace::run(&env).await,
        Commands::Bench => commands::bench::run().await,
        Commands::Env { command } => match command {
            EnvCommands::Ls => commands::env::ls().await,
            EnvCommands::Url { env } => commands::env::url(&env).await,
        },
        Commands::Docs => commands::docs::run().await,
        Commands::Mcp => commands::mcp::run().await,
    }
}
