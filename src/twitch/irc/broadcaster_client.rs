use super::client::{TwitchIRCManager, TwitchIRCClientType};
use std::sync::Arc;

pub struct TwitchBroadcasterClient {
    username: String,
    manager: Arc<TwitchIRCManager>,
}

impl TwitchBroadcasterClient {
    pub fn new(username: String, manager: Arc<TwitchIRCManager>) -> Self {
        TwitchBroadcasterClient { username, manager }
    }

    pub async fn get_client(&self) -> Option<Arc<TwitchIRCClientType>> {
        self.manager.get_client(&self.username).await
    }

    pub async fn send_message(&self, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.manager.send_message(&self.username, channel, message).await
    }

    // Add more broadcaster-specific methods here
}