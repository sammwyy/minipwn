mod agent;
mod ai;
mod config;
mod protocol;
mod tools;
mod tui;
mod worker;
mod commands;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "minipwn")]
#[command(about = "Autonomous pentesting agent")]
#[command(version = "0.1.0")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the worker server
    #[command(alias = "w")]
    Worker {
        /// Authentication secret (overrides config)
        #[arg(long)]
        secret: Option<String>,

        /// Port to listen on (overrides config)
        #[arg(long)]
        port: Option<u16>,

        /// Path to worker config file
        #[arg(long)]
        config: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Init logging (suppressed by default; enable with RUST_LOG=debug)
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "minipwn=warn".into()),
        )
        .init();

    // Ensure global config directories and default files exist
    config::init_config_dirs()?;

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Worker {
            secret,
            port,
            config,
        }) => {
            worker::server::run(secret, port, config).await?;
        }
        None => {
            tui::run().await?;
        }
    }

    Ok(())
}
