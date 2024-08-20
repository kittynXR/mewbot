use super::client::TwitchIRCManager;
use super::command_system::COMMANDS;
use crate::config::Config;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::roles::get_user_role;
use crate::twitch::redeems::RedeemManager;
use crate::storage::StorageClient;
use crate::twitch::role_cache::RoleCache;
use crate::discord::UserLinks;
use std::sync::Arc;
use std::time::Duration;
use log::{debug, error, warn};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::timeout;
use twitch_irc::message::ServerMessage;
use uuid::Uuid;
use crate::ai::AIClient;
use crate::twitch::irc::TwitchBotClient;
use crate::vrchat::{VRChatClient, World};
use crate::web_ui::websocket::WebSocketMessage;

pub struct MessageHandler {
    irc_client: Arc<TwitchBotClient>,
    config: Arc<RwLock<Config>>,
    api_client: Arc<TwitchAPIClient>,
    redeem_manager: Arc<RwLock<RedeemManager>>,
    storage: Arc<RwLock<StorageClient>>,
    role_cache: Arc<RwLock<RoleCache>>,
    user_links: Arc<UserLinks>,
    websocket_sender: mpsc::Sender<WebSocketMessage>,
    world_info: Arc<Mutex<Option<World>>>,
    vrchat_client: Arc<VRChatClient>,
    ai_client: Option<Arc<AIClient>>,
}

impl MessageHandler {
    pub fn new(
        irc_client: Arc<TwitchBotClient>,
        config: Arc<RwLock<Config>>,
        api_client: Arc<TwitchAPIClient>,
        redeem_manager: Arc<RwLock<RedeemManager>>,
        storage: Arc<RwLock<StorageClient>>,
        role_cache: Arc<RwLock<RoleCache>>,
        user_links: Arc<UserLinks>,
        websocket_sender: mpsc::Sender<WebSocketMessage>,
        world_info: Arc<Mutex<Option<World>>>,
        vrchat_client: Arc<VRChatClient>,
        ai_client: Option<Arc<AIClient>>,
    ) -> Self {
        MessageHandler {
            irc_client,
            config,
            api_client,
            redeem_manager,
            storage,
            role_cache,
            user_links,
            websocket_sender,
            world_info,
            vrchat_client,
            ai_client,
        }
    }

    pub async fn handle_messages(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut receiver = self.irc_client.subscribe();

        while let Ok(message) = receiver.recv().await {
            debug!("Received message in handle_messages: {:?}", message);
            debug!("Received Twitch message: {:?}", message);
            if let Err(e) = self.handle_message(message).await {
                error!("Error handling message: {:?}", e);
            }
        }
        Ok(())
    }

    pub async fn handle_message(&self, message: ServerMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let debug_id = Uuid::new_v4();
        warn!("Starting handle_message (Debug ID: {})", debug_id);
        debug!("Processing message in handle_message: {:?} (Debug ID: {})", message, debug_id);

        if let ServerMessage::Privmsg(msg) = message {
            let channel_id = self.api_client.get_broadcaster_id().await?;
            let user_role = get_user_role(&msg.sender.id, &channel_id, &self.api_client, &self.storage, &self.role_cache).await?;

            let cleaned_message = msg.message_text
                .chars()
                .filter(|&c| c.is_ascii_graphic() || c.is_ascii_whitespace())
                .collect::<String>()
                .trim()
                .to_string();

            warn!("Cleaned message: {} (Debug ID: {})", cleaned_message, debug_id);

            // Send the message to the WebSocket
            let websocket_message = WebSocketMessage {
                message_type: "twitch_message".to_string(),
                message: Some(cleaned_message.clone()),
                user_id: Some(msg.sender.id.clone()),
                destination: None,
                world: None,
                additional_streams: None,
            };
            if let Err(e) = self.websocket_sender.send(websocket_message).await {
                error!("Failed to send message to WebSocket: {:?} (Debug ID: {})", e, debug_id);
            } else {
                debug!("Successfully sent message to WebSocket (Debug ID: {})", debug_id);
            }

            let lowercase_message = cleaned_message.to_lowercase();
            let mut parts = lowercase_message.split_whitespace();
            let command = parts.next();
            let params: Vec<&str> = parts.collect();

            if let Some(cmd) = command {
                warn!("Identified command: {} (Debug ID: {})", cmd, debug_id);
                if let Some(command) = COMMANDS.iter().find(|c| c.name == cmd) {
                    warn!("Found matching command: {} (Debug ID: {})", command.name, debug_id);
                    if user_role >= command.required_role {
                        let broadcaster_id = self.api_client.get_broadcaster_id().await?;
                        let is_stream_online = self.api_client.is_stream_live(&broadcaster_id).await?;
                        warn!("Executing command: {} (Debug ID: {})", command.name, debug_id);
                        (command.handler)(
                            &msg,
                            &self.irc_client,
                            &msg.channel_login,
                            &self.api_client,
                            &self.world_info,
                            &Arc::new(Mutex::new(super::commands::ShoutoutCooldown::new())),
                            &self.redeem_manager,
                            &self.role_cache,
                            &self.storage,
                            &self.user_links,
                            &params,
                            &self.config,
                            &self.vrchat_client,
                            &self.ai_client,
                            is_stream_online
                        ).await?;
                        warn!("Finished executing command: {} (Debug ID: {})", command.name, debug_id);
                    } else {
                        let response = format!("@{}, this command is only available to {:?}s and above.", msg.sender.name, command.required_role);
                        self.irc_client.send_message(&msg.channel_login, &response).await?;
                        warn!("User does not have required role for command: {} (Debug ID: {})", command.name, debug_id);
                    }
                } else {
                    warn!("No matching command found for: {} (Debug ID: {})", cmd, debug_id);
                }
            }
        }
        warn!("Completed handle_message (Debug ID: {})", debug_id);
        Ok(())
    }
}