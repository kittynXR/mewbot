use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::Local;
use colored::*;
use serde::{Deserialize, Serialize};
use crate::config::Config;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub enum LogLevel {
    ERROR = 0,
    WARN = 1,
    INFO = 2,
    DEBUG = 3,
    VERBOSE = 4,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::INFO
    }
}

impl LogLevel {
    pub fn includes(&self, other: LogLevel) -> bool {
        *self as u8 >= other as u8
    }
}

pub struct Logger {
    config: Arc<RwLock<Config>>,
}

impl Logger {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        Logger { config }
    }

    pub fn log(&self, level: LogLevel, message: &str) {
        let config = self.config.try_read().unwrap_or_else(|_| panic!("Failed to read config"));
        if config.log_level.includes(level) {
            let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let level_str = match level {
                LogLevel::ERROR => "ERROR".red(),
                LogLevel::WARN => "WARN".yellow(),
                LogLevel::INFO => "INFO".green(),
                LogLevel::DEBUG => "DEBUG".blue(),
                LogLevel::VERBOSE => "VERBOSE".cyan(),
            };
            println!("[{}] [{}] {}", timestamp, level_str, message);
        }
    }
}

#[macro_export]
macro_rules! log_error {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log(LogLevel::ERROR, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_warn {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log(LogLevel::WARN, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_info {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log(LogLevel::INFO, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_debug {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log(LogLevel::DEBUG, &format!($($arg)*))
    };
}

#[macro_export]
macro_rules! log_verbose {
    ($logger:expr, $($arg:tt)*) => {
        $logger.log(LogLevel::VERBOSE, &format!($($arg)*))
    };
}

pub async fn set_debug_logging(config: &Arc<RwLock<Config>>, debug: bool) {
    let mut config = config.write().await;
    config.log_level = if debug { LogLevel::DEBUG } else { LogLevel::INFO };
    config.save().unwrap_or_else(|e| eprintln!("Failed to save config: {}", e));
}

pub async fn set_verbose_logging(config: &Arc<RwLock<Config>>, verbose: bool) {
    let mut config = config.write().await;
    config.log_level = if verbose { LogLevel::VERBOSE } else { LogLevel::INFO };
    config.save().unwrap_or_else(|e| eprintln!("Failed to save config: {}", e));
}