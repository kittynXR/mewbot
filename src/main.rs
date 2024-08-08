use clap::Parser;
use mewbot::{config::Config, init, run};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use mewbot::logging::LogLevel;

/// MewBot - A Twitch and VRChat bot
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    /// Set log level (error, warn, info, debug, verbose)
    #[arg(short = 'L', long, value_name = "LEVEL")]
    log_level: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let config_path = args.config.unwrap_or_else(|| "mewbot.conf".to_string());
    let mut config = Config::new()?;

    if let Some(level) = args.log_level {
        let new_level = match level.to_lowercase().as_str() {
            "error" => LogLevel::ERROR,
            "warn" => LogLevel::WARN,
            "info" => LogLevel::INFO,
            "debug" => LogLevel::DEBUG,
            "verbose" => LogLevel::VERBOSE,
            _ => {
                eprintln!("Invalid log level. Using default (INFO).");
                LogLevel::INFO
            }
        };
        config.log_level = new_level;
        config.save()?;
    }

    let config = Arc::new(RwLock::new(config));

    println!("Log level: {:?}", config.read().await.log_level);

    let clients = init(Arc::clone(&config)).await?;

    // Initialize the RedeemManager with current status
    clients.redeem_manager.write().await.initialize_with_current_status().await?;
    // clients.eventsub_client.check_current_stream_status().await?;

    run(clients, config).await?;

    Ok(())
}