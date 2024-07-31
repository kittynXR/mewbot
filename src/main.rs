use clap::Parser;
use mewbot::{config::Config, init, run, logging::set_verbose_logging};
use std::sync::Arc;
use tokio::sync::RwLock;

/// MewBot - A Twitch and VRChat bot
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    /// Turn on verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let config_path = args.config.unwrap_or_else(|| "mewbot.conf".to_string());
    let config = Config::new()?;
    let config = Arc::new(RwLock::new(config));

    if args.verbose {
        set_verbose_logging(&config, true).await;
    }


    let clients = init(Arc::clone(&config)).await?;

    // Initialize the RedeemManager with current status
    clients.redeem_manager.write().await.initialize_with_current_status().await?;

    // Check current stream status
    // clients.eventsub_client.lock().await.check_current_stream_status().await?;

    run(clients, config).await?;

    Ok(())
}