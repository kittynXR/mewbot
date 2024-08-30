use super::command_system::{CommandContext, CommandRegistry};
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
use crate::twitch::irc::commands::{
    PingCommand,
    CalcCommand,
    DiscordCommand,
    IsItFridayCommand,
    XmasCommand,
    ShoutoutCommand,
    UptimeCommand,
    FollowersCommand, FollowAgeCommand,
    VerifyCommand,
    VRCCommand,
    WorldCommand,
};


pub struct MessageHandler {
    config: Arc<RwLock<Config>>,
    pub(crate) twitch_manager: Arc<TwitchManager>,
    storage: Arc<RwLock<StorageClient>>,
    websocket_sender: mpsc::UnboundedSender<WebSocketMessage>,
    world_info: Arc<Mutex<Option<World>>>,
    vrchat_manager: Arc<VRChatManager>,
    ai_client: Option<Arc<AIClient>>,
    command_registry: CommandRegistry,
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
        let mut command_registry = CommandRegistry::new();

        // Register commands here
        command_registry.register(Box::new(PingCommand));
        command_registry.register(Box::new(CalcCommand));
        command_registry.register(Box::new(DiscordCommand));
        command_registry.register(Box::new(FollowersCommand));
        command_registry.register(Box::new(FollowAgeCommand));
        command_registry.register(Box::new(IsItFridayCommand));
        command_registry.register(Box::new(XmasCommand));
        command_registry.register(Box::new(ShoutoutCommand));
        command_registry.register(Box::new(UptimeCommand));
        command_registry.register(Box::new(VerifyCommand));
        command_registry.register(Box::new(VRCCommand));
        command_registry.register(Box::new(WorldCommand));


        MessageHandler {
            config,
            twitch_manager,
            storage,
            websocket_sender,
            world_info,
            vrchat_manager,
            ai_client,
            command_registry,
        }
    }

    pub async fn handle_messages(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut receiver = self.twitch_manager.irc_manager.subscribe();

        while let Ok(message) = receiver.recv().await {
            if let Err(e) = self.handle_message(message).await {
                error!("Error handling message: {:?}", e);
            }
        }

        Ok(())
    }

    pub(crate) async fn handle_message(&self, message: ServerMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Processing message in handle_message: {:?}", message);

        if let ServerMessage::Privmsg(msg) = message {
            let cleaned_message = msg.message_text
                .chars()
                .filter(|&c| !c.is_control() && !c.is_whitespace() || c.is_ascii_whitespace())
                .collect::<String>()
                .trim()
                .to_string();

            let mut parts = cleaned_message.split_whitespace();
            let command = parts.next();
            let args: Vec<String> = parts.map(String::from).collect();

            if let Some(cmd) = command {
                let ctx = CommandContext {
                    msg: msg.clone(),
                    bot_client: self.twitch_manager.get_bot_client(),
                    channel: msg.channel_login.clone(),
                    twitch_manager: self.twitch_manager.clone(),
                    world_info: self.world_info.clone(),
                    redeem_manager: self.twitch_manager.get_redeem_manager(),
                    storage: self.storage.clone(),
                    user_links: self.twitch_manager.get_user_links(),
                    config: self.config.clone(),
                    vrchat_manager: self.vrchat_manager.clone(),
                    ai_client: self.ai_client.clone(),
                    is_stream_online: self.twitch_manager.is_stream_live().await,
                };

                self.command_registry.execute(cmd, &ctx, args).await?;
            }
        }
        Ok(())
    }
}