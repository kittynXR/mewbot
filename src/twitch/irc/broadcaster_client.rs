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
        let client = self.get_client().await.ok_or("Client not initialized")?;
        client.say(channel.to_string(), message.to_string()).await?;
        Ok(())
    }

    // Add more broadcaster-specific methods here
}