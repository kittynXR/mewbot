use std::time::{Duration, Instant};
use log::{info, warn, error};

pub struct ConnectionMonitor {
    last_connected: Option<Instant>,
    last_message_received: Option<Instant>,
    pub(crate) disconnection_count: u32,
    pub(crate) total_uptime: Duration,
    connection_state: ConnectionState,
}

pub enum ConnectionState {
    Connected,
    Disconnected,
    Reconnecting,
}

impl ConnectionMonitor {
    pub fn new() -> Self {
        Self {
            last_connected: None,
            last_message_received: None,
            disconnection_count: 0,
            total_uptime: Duration::from_secs(0),
            connection_state: ConnectionState::Disconnected,
        }
    }

    pub fn on_connect(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_connected {
            self.total_uptime += now - last;
        }
        self.last_connected = Some(now);
        self.connection_state = ConnectionState::Connected;
        info!("Twitch IRC connected. Total uptime: {:?}, Disconnection count: {}",
              self.total_uptime, self.disconnection_count);
    }

    pub fn on_disconnect(&mut self) {
        self.disconnection_count += 1;
        if let Some(last) = self.last_connected.take() {
            self.total_uptime += Instant::now() - last;
        }
        self.connection_state = ConnectionState::Disconnected;
        warn!("Twitch IRC disconnected. Total uptime: {:?}, Disconnection count: {}",
              self.total_uptime, self.disconnection_count);
    }

    pub fn on_message_received(&mut self) {
        self.last_message_received = Some(Instant::now());
    }

    pub fn start_reconnecting(&mut self) {
        self.connection_state = ConnectionState::Reconnecting;
        warn!("Starting reconnection process for Twitch IRC");
    }

    pub fn is_connection_stale(&self, timeout: Duration) -> bool {
        match self.last_message_received {
            Some(last) => Instant::now().duration_since(last) > timeout,
            None => true,
        }
    }
}