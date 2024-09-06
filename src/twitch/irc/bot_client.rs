use super::client::{TwitchIRCManager, TwitchIRCClientType};
use std::sync::Arc;
use log::{ error, warn};
use tokio::sync::broadcast;
use twitch_irc::message::ServerMessage;

pub struct TwitchBotClient {
    username: String,
    manager: Arc<TwitchIRCManager>,
}

impl Default for TwitchBotClient {
    fn default() -> Self {
        Self {
            username: String::new(),
            manager: Arc::new(TwitchIRCManager::default()),
        }
    }
}

impl TwitchBotClient {
    pub fn new(username: String, manager: Arc<TwitchIRCManager>) -> Self {
        TwitchBotClient { username, manager }
    }

    pub async fn get_client(&self) -> Option<Arc<TwitchIRCClientType>> {
        self.manager.get_client(&self.username).await
    }

    pub async fn send_message(&self, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        warn!("TwitchBotClient: Attempting to send message to channel {}: {}", channel, message);
        match self.manager.send_message(&self.username, channel, message).await {
            Ok(_) => {
                warn!("TwitchBotClient: Message sent successfully: {}", message);
                Ok(())
            },
            Err(e) => {
                error!("TwitchBotClient: Error sending message: {:?}", e);
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