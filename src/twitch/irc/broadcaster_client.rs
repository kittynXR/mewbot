use super::client::{TwitchIRCManager, TwitchIRCClientType};
use std::sync::Arc;
use log::{error, warn};
use tokio::sync::broadcast;
use twitch_irc::message::ServerMessage;

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
        warn!("TwitchBroadcasterClient: Attempting to send message to channel {}: {}", channel, message);
        match self.manager.send_message(&self.username, channel, message).await {
            Ok(_) => {
                warn!("TwitchBroadcasterClient: Message sent successfully: {}", message);
                Ok(())
            },
            Err(e) => {
                error!("TwitchBroadcasterClient: Error sending message: {:?}", e);
                Err(e)
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.manager.subscribe()
    }

    pub fn get_manager(&self) -> Arc<TwitchIRCManager> {
        self.manager.clone()
    }

}