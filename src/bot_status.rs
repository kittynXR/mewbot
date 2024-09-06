use std::time::{Instant, Duration};
use tokio::sync::RwLock;
use std::sync::Arc;
use log::info;

pub struct BotStatus {
    start_time: Instant,
    is_online: bool,
}

impl Default for BotStatus {
    fn default() -> Self {
        Self {
            // Initialize with default values
            start_time: std::time::Instant::now(),
            is_online: false,
        }
    }
}

impl BotStatus {
    pub fn new() -> Arc<RwLock<Self>> {
        Arc::new(RwLock::new(Self {
            start_time: Instant::now(),
            is_online: false,
        }))
    }

    pub fn set_online(&mut self, online: bool) {
        self.is_online = online;
        if online {
            self.start_time = Instant::now();
        }
        info!("Bot status changed to: {}", if online { "online" } else { "offline" });
    }

    pub fn is_online(&self) -> bool {
        self.is_online
    }

    pub fn uptime(&self) -> Duration {
        if self.is_online {
            Instant::now().duration_since(self.start_time)
        } else {
            Duration::from_secs(0)
        }
    }

    pub fn uptime_string(&self) -> String {
        let uptime = self.uptime();
        let seconds = uptime.as_secs();
        let (hours, minutes, seconds) = (seconds / 3600, (seconds % 3600) / 60, seconds % 60);
        format!("{}h {}m {}s", hours, minutes, seconds)
    }
}