use std::time::{Duration, Instant};
use log::{info, warn, error};

pub struct ConnectionMonitor {
    last_connected: Option<Instant>,
    disconnection_count: u32,
    total_uptime: Duration,
}

impl ConnectionMonitor {
    pub fn new() -> Self {
        Self {
            last_connected: None,
            disconnection_count: 0,
            total_uptime: Duration::from_secs(0),
        }
    }

    pub fn on_connect(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_connected {
            self.total_uptime += now - last;
        }
        self.last_connected = Some(now);
        info!("Twitch IRC connected. Total uptime: {:?}, Disconnection count: {}",
              self.total_uptime, self.disconnection_count);
    }

    pub fn on_disconnect(&mut self) {
        self.disconnection_count += 1;
        if let Some(last) = self.last_connected.take() {
            self.total_uptime += Instant::now() - last;
        }
        warn!("Twitch IRC disconnected. Total uptime: {:?}, Disconnection count: {}",
              self.total_uptime, self.disconnection_count);
    }
}