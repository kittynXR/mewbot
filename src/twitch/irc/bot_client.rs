use super::client::{TwitchIRCManager, TwitchIRCClientType};
use std::sync::Arc;
use log::{debug, error, trace};
use tokio::sync::broadcast;
use twitch_irc::message::ServerMessage;

pub struct TwitchBotClient {
    username: String,
    manager: Arc<TwitchIRCManager>,
}

impl TwitchBotClient {
    pub fn new(username: String, manager: Arc<TwitchIRCManager>) -> Self {
        TwitchBotClient { username, manager }
    }

    pub async fn get_client(&self) -> Option<Arc<TwitchIRCClientType>> {
        self.manager.get_client(&self.username).await
    }

    pub async fn send_message(&self, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Attempting to send message to channel {}: {}", channel, message);
        match self.manager.send_message(&self.username, channel, message).await {
            Ok(_) => {
                trace!("Message sent successfully");
                Ok(())
            },
            Err(e) => {
                error!("Error sending message: {:?}", e);
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