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
        debug!("Processing message in handle_message: {:?}", message);

        if let ServerMessage::Privmsg(msg) = message {
            let channel_id = self.api_client.get_broadcaster_id().await?;
            let user_role = get_user_role(&msg.sender.id, &channel_id, &self.api_client, &self.storage, &self.role_cache).await?;

            let cleaned_message = msg.message_text
                .chars()
                .filter(|&c| c.is_ascii_graphic() || c.is_ascii_whitespace())
                .collect::<String>()
                .trim()
                .to_string();


            let lowercase_message = cleaned_message.to_lowercase();
            let mut parts = lowercase_message.split_whitespace();
            let command = parts.next();
            let params: Vec<&str> = parts.collect();

            if let Some(cmd) = command {
                if let Some(command) = COMMANDS.iter().find(|c| c.name == cmd) {
                    if user_role >= command.required_role {
                        let broadcaster_id = self.api_client.get_broadcaster_id().await?;
                        let is_stream_online = self.api_client.is_stream_live(&broadcaster_id).await?;
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
                    } else {
                        let response = format!("@{}, this command is only available to {:?}s and above.", msg.sender.name, command.required_role);
                        self.irc_client.send_message(&msg.channel_login, &response).await?;
                    }
                } else {
                }
            }
        }
        Ok(())
    }
}