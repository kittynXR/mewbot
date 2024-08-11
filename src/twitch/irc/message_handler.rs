use super::client::TwitchIRCManager;
use super::command_system::COMMANDS;
use crate::config::Config;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::roles::get_user_role;
use crate::twitch::redeems::RedeemManager;
use crate::storage::StorageClient;
use crate::twitch::role_cache::RoleCache;
use crate::discord::UserLinks;
use crate::logging::Logger;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use twitch_irc::message::ServerMessage;
use crate::twitch::irc::TwitchBotClient;

pub struct MessageHandler {
    irc_client: Arc<TwitchBotClient>,
    config: Arc<RwLock<Config>>,
    api_client: Arc<TwitchAPIClient>,
    redeem_manager: Arc<RwLock<RedeemManager>>,
    storage: Arc<RwLock<StorageClient>>,
    role_cache: Arc<RwLock<RoleCache>>,
    user_links: Arc<UserLinks>,
    logger: Arc<Logger>,
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
        logger: Arc<Logger>,
    ) -> Self {
        MessageHandler {
            irc_client,
            config,
            api_client,
            redeem_manager,
            storage,
            role_cache,
            user_links,
            logger,
        }
    }

    pub async fn handle_messages(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut receiver = self.irc_client.subscribe();

        while let Ok(message) = receiver.recv().await {
            self.handle_message(message).await?;
        }

        Ok(())
    }

    pub async fn handle_message(&self, message: ServerMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                        (command.handler)(
                            &msg,
                            &self.irc_client,  // Pass the TwitchBotClient directly
                            &msg.channel_login,
                            &self.api_client,
                            &Arc::new(Mutex::new(None)), // world_info is not used in this example
                            &Arc::new(Mutex::new(super::commands::ShoutoutCooldown::new())),
                            &self.redeem_manager,
                            &self.role_cache,
                            &self.storage,
                            &self.user_links,
                            &params,
                            &self.config,
                            &self.logger
                        ).await?;
                    } else {
                        let response = format!("@{}, this command is only available to {:?}s and above.", msg.sender.name, command.required_role);
                        self.irc_client.send_message(&msg.channel_login, &response).await?;
                    }
                }
            }
        }
        Ok(())
    }
}