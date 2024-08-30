use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use twitch_irc::message::PrivmsgMessage;

use crate::ai::AIClient;
use crate::config::Config;
use crate::discord::UserLinks;
use crate::storage::StorageClient;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::manager::TwitchManager;
use crate::twitch::redeems::RedeemManager;
use crate::twitch::roles::UserRole;
use crate::vrchat::VRChatManager;
use crate::vrchat::models::World;

pub struct CommandContext {
    pub msg: PrivmsgMessage,
    pub bot_client: Arc<TwitchBotClient>,
    pub channel: String,
    pub twitch_manager: Arc<TwitchManager>,
    pub world_info: Arc<Mutex<Option<World>>>,
    pub redeem_manager: Arc<RwLock<RedeemManager>>,
    pub storage: Arc<RwLock<StorageClient>>,
    pub user_links: Arc<UserLinks>,
    pub config: Arc<RwLock<Config>>,
    pub vrchat_manager: Arc<VRChatManager>,
    pub ai_client: Option<Arc<AIClient>>,
    pub is_stream_online: bool,
}

#[async_trait::async_trait]
pub trait Command: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn execute(&self, ctx: &CommandContext, args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn required_role(&self) -> UserRole;
}

pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    pub fn register(&mut self, command: Box<dyn Command>) {
        self.commands.insert(command.name().to_string(), command);
    }

    pub async fn execute(&self, name: &str, ctx: &CommandContext, args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(command) = self.commands.get(name) {
            let user_role = ctx.twitch_manager.get_user(&ctx.msg.sender.id).await?.role;
            if user_role >= command.required_role() {
                command.execute(ctx, args).await
            } else {
                let response = format!("@{}, this command is only available to {:?}s and above.", ctx.msg.sender.name, command.required_role());
                ctx.bot_client.send_message(&ctx.channel, &response).await?;
                Ok(())
            }
        } else {
            Ok(())
        }
    }
}