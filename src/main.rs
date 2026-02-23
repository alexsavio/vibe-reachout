mod bot;
mod config;
mod error;
mod hook;
mod install;
mod ipc;
mod models;
mod telegram;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(
    name = "vibe-reachout",
    about = "Telegram permission hook for Claude Code"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Telegram bot (long-running)
    Bot,
    /// Register the permission hook in Claude Code settings
    Install,
}

fn init_tracing(is_hook_mode: bool) {
    let default_level = if is_hook_mode { "warn" } else { "info" };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_level));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

fn main() {
    let cli = Cli::parse();

    let is_hook_mode = cli.command.is_none();
    init_tracing(is_hook_mode);

    match cli.command {
        Some(Commands::Bot) => {
            let config = match config::Config::load() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(1);
                }
            };

            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            if let Err(e) = rt.block_on(bot::run_bot(config)) {
                eprintln!("Bot error: {e}");
                std::process::exit(1);
            }
        }
        Some(Commands::Install) => {
            if let Err(e) = install::run_install() {
                eprintln!("Install error: {e}");
                std::process::exit(1);
            }
        }
        None => {
            // Hook mode: read stdin, forward to bot, write stdout
            let config = match config::Config::load() {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!("Config error: {e}");
                    std::process::exit(1);
                }
            };

            let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
            if let Err(e) = rt.block_on(hook::run_hook(&config)) {
                tracing::warn!("Hook error: {e}");
                std::process::exit(1);
            }
        }
    }
}
