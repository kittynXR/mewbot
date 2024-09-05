use clap::{Parser, ArgAction};
use mewbot::{config::Config, init, run};
use std::sync::Arc;
use tokio::sync::RwLock;
use log::{error, info, LevelFilter};
use fern::colors::{Color, ColoredLevelConfig};
use chrono::Local;
use std::{fs, panic};
use std::path::Path;

/// MewBot - A Twitch and VRChat bot
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Sets a custom config file
    #[arg(short, long, value_name = "FILE")]
    config: Option<String>,

    /// Set log level (error, warn, info, debug, trace)
    #[arg(short = 'L', long, value_name = "LEVEL")]
    log_level: Option<String>,

    /// Enable single-level logging (only show logs of the specified level)
    #[arg(long, action = ArgAction::SetTrue)]
    single_level: bool,
}

fn setup_logger(log_level: LevelFilter, single_level: bool) -> Result<(), fern::InitError> {
    // Configure colors for log levels
    let colors = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::Blue)
        .trace(Color::Magenta);

    // Create logs directory if it doesn't exist
    let logs_dir = Path::new("logs");
    fs::create_dir_all(logs_dir)?;

    // Generate a unique log file name based on the current date and time
    let log_file_name = Local::now().format("mewbot_%Y-%m-%d_%H-%M-%S.log").to_string();
    let log_file_path = logs_dir.join(log_file_name);

    // Define a list of modules to filter out
    let filtered_modules = vec![
        "tokio_tungstenite",
        "tungstenite",
        "hyper_util",
        "serenity",
        "tracing::span",
    ];

    // Build the logger
    let dispatch = fern::Dispatch::new()
        .format(move |out, message, record| {
            if !single_level || record.level() == log_level {
                out.finish(format_args!(
                    "{}[{}][{}] {}",
                    Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                    record.target(),
                    colors.color(record.level()),
                    message
                ))
            }
        })
        .level(log_level)
        .level_for("serenity", LevelFilter::Error)
        .filter(move |metadata| {
            // Filter out specified modules
            !filtered_modules.iter().any(|&module| metadata.target().contains(module))
                && !(metadata.level() <= log::Level::Info && metadata.target().contains("do_heartbeat"))
        })
        .chain(std::io::stdout())
        .chain(fern::log_file(log_file_path)?);

    // Apply the logger configuration
    dispatch.apply()?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Args::parse();

    let mut config = Config::new()?;

    // Set log level
    let log_level = if let Some(level) = args.log_level {
        match level.to_lowercase().as_str() {
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "info" => LevelFilter::Info,
            "debug" => LevelFilter::Debug,
            "trace" => LevelFilter::Trace,
            _ => {
                eprintln!("Invalid log level. Using default (INFO).");
                LevelFilter::Info
            }
        }
    } else {
        config.log_level
    };

    // Initialize logger
    setup_logger(log_level, args.single_level)?;

    // Update config with new log level and save
    config.log_level = log_level;
    config.save()?;

    let config = Arc::new(RwLock::new(config));

    info!("Starting MewBot with log level: {:?}", log_level);
    if args.single_level {
        info!("Single-level logging enabled. Only showing logs at the {} level.", log_level);
    }

    let clients = init(Arc::clone(&config)).await?;

    // Initialize the RedeemManager with current status
    // clients.twitch_manager.redeem_manager.write().await.initialize_with_current_status().await?;

    panic::set_hook(Box::new(|panic_info| {
        error!("A panic occurred: {:?}", panic_info);
    }));

    run(clients, config).await?;

    info!("MewBot shutting down");
    Ok(())
}