use crate::config::Config;
use std::sync::Arc;
use tokio::sync::RwLock;

#[macro_export]
macro_rules! log_verbose {
    ($config:expr, $($arg:tt)*) => {
        if $config.read().await.verbose_logging {
            println!("[VERBOSE] {}", format!($($arg)*));
        }
    };
}

pub async fn set_verbose_logging(config: &Arc<RwLock<Config>>, verbose: bool) {
    let mut config = config.write().await;
    config.verbose_logging = verbose;
    config.save().unwrap_or_else(|e| eprintln!("Failed to save config: {}", e));
}