use super::client::TwitchIRCManager;
use super::command_system::COMMANDS;
use crate::config::Config;
use crate::twitch::redeems::RedeemManager;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use std::sync::Arc;
use log::{debug, error};
use tokio::sync::{mpsc, Mutex, RwLock};
use twitch_irc::message::ServerMessage;
use crate::ai::AIClient;
use crate::twitch::irc::TwitchBotClient;
use crate::vrchat::{VRChatManager, World};
use crate::web_ui::websocket::WebSocketMessage;
use crate::twitch::manager::TwitchManager;

pub struct MessageHandler {
    config: Arc<RwLock<Config>>,
    twitch_manager: Arc<TwitchManager>,
    storage: Arc<RwLock<StorageClient>>,
    websocket_sender: mpsc::UnboundedSender<WebSocketMessage>,
    world_info: Arc<Mutex<Option<World>>>,
    vrchat_manager: Arc<VRChatManager>,
    ai_client: Option<Arc<AIClient>>,
}

impl MessageHandler {
    pub fn new(
        config: Arc<RwLock<Config>>,
        twitch_manager: Arc<TwitchManager>,
        storage: Arc<RwLock<StorageClient>>,
        websocket_sender: mpsc::UnboundedSender<WebSocketMessage>,
        world_info: Arc<Mutex<Option<World>>>,
        vrchat_manager: Arc<VRChatManager>,
        ai_client: Option<Arc<AIClient>>,
    ) -> Self {
        MessageHandler {
            config,
            twitch_manager,
            storage,
            websocket_sender,
            world_info,
            vrchat_manager,
            ai_client,
        }
    }

    pub async fn handle_messages(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // let mut receiver = self.twitch_manager.irc_manager.subscribe();

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        }

        // Err("Twitch IRC message channel closed unexpectedly".into())
    }

    pub async fn handle_message(&self, message: ServerMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Processing message in handle_message: {:?}", message);

        if let ServerMessage::Privmsg(msg) = message {
            let user = self.twitch_manager.get_user(&msg.sender.id).await?;
            let user_role = user.role;

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
                        let is_stream_online = self.twitch_manager.is_stream_live();
                        (command.handler)(
                            &msg,
                            &self.twitch_manager.get_bot_client(),
                            &msg.channel_login,
                            &self.twitch_manager,
                            &self.world_info,
                            &Arc::new(Mutex::new(super::commands::ShoutoutCooldown::new())),
                            &self.twitch_manager.get_redeem_manager(),
                            &self.storage,
                            &self.twitch_manager.get_user_links(),
                            &params,
                            &self.config,
                            &self.vrchat_manager,
                            &self.ai_client,
                            is_stream_online.await
                        ).await?;
                    } else {
                        let response = format!("@{}, this command is only available to {:?}s and above.", msg.sender.name, command.required_role);
                        self.twitch_manager.send_message_as_bot(&msg.channel_login, &response).await?;
                    }
                }
            }
        }
        Ok(())
    }
}