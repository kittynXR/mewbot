use crate::twitch::roles::UserRole;
use crate::vrchat::models::World;
use crate::twitch::api::TwitchAPIClient;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use twitch_irc::message::PrivmsgMessage;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::future::Future;
use std::pin::Pin;
use crate::ai::AIClient;
use crate::twitch::redeems::RedeemManager;
use crate::twitch::role_cache::RoleCache;
use crate::config::Config;
use crate::twitch::irc::TwitchBotClient;
use crate::vrchat::{VRChatClient, VRChatManager};
use super::commands;

pub struct Command {
    pub name: &'static str,
    pub required_role: UserRole,
    pub handler: for<'a> fn(
        &'a PrivmsgMessage,
        &'a Arc<TwitchBotClient>,
        &'a str,
        &'a Arc<TwitchAPIClient>,
        &'a Arc<Mutex<Option<World>>>,
        &'a Arc<Mutex<commands::ShoutoutCooldown>>,
        &'a Arc<RwLock<RedeemManager>>,
        &'a Arc<RwLock<RoleCache>>,
        &'a Arc<RwLock<StorageClient>>,
        &'a Arc<UserLinks>,
        &'a [&'a str],
        &'a Arc<RwLock<Config>>,
        &'a Arc<VRChatManager>,
        &'a Option<Arc<AIClient>>,
        bool
    ) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>>,
    pub description: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "!world",
        required_role: UserRole::Subscriber,
        handler: |msg, client, channel, _api_client, world_info, _cooldowns, _redeem_manager, _role_cache, storage, user_links, _params, _config, vrchat_client, _ai_client, is_stream_online|
            Box::pin(commands::handle_world(msg, client, channel, world_info, storage, user_links, vrchat_client, is_stream_online)),
        description: "Shows information about the current VRChat world",
    },
    Command {
        name: "!uptime",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, api_client, _world_info, _cooldowns, _redeem_manager, _role_cache, storage, user_links, _params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_uptime(msg, client, channel, api_client, storage, user_links)),
        description: "Shows how long the stream has been live",
    },
    Command {
        name: "!ping",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, _api_client, _world_info, _cooldowns, _redeem_manager, _role_cache, storage, user_links, _params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_ping(msg, client, channel, storage, user_links)),
        description: "Responds with Pong!",
    },
    Command {
        name: "!so",
        required_role: UserRole::Subscriber,
        handler: |msg, client, channel, api_client, _world_info, cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_shoutout(msg, client, channel, api_client, cooldowns, params, redeem_manager, storage, user_links)),
        description: "Gives a shoutout to another streamer",
    },
    Command {
        name: "!clearcache",
        required_role: UserRole::Broadcaster,
        handler: |_msg, client, channel, _api_client, _world_info, _cooldowns, _redeem_manager, role_cache, _storage, _user_links, _params, _config, _vrchat_client, _ai_client, _is_stream_online| Box::pin(async move {
            role_cache.write().await.clear();
            client.send_message(channel, "Role cache has been cleared.").await?;
            Ok(())
        }),
        description: "Clears the role cache",
    },
    Command {
        name: "!complete",
        required_role: UserRole::Subscriber,
        handler: |msg, client, channel, _api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_complete_redemption(msg, client, channel, redeem_manager, storage, user_links, params)),
        description: "Marks a queued redemption as complete",
    },
    Command {
        name: "!add_redeem",
        required_role: UserRole::Moderator,
        handler: |msg, client, channel, api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_add_redeem(msg, client, channel, api_client, redeem_manager, storage, user_links, params)),
        description: "Adds a new channel point redemption. Usage: !add_redeem \"<title>\" <cost> <action_type> <cooldown> \"<prompt>\" [queued] [announce]",
    },
    Command {
        name: "!setactivegames",
        required_role: UserRole::Moderator,
        handler: |msg, client, channel, _api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_set_active_games(msg, client, channel, redeem_manager, storage, user_links, params)),
        description: "Sets the games for which a redeem is active",
    },
    Command {
        name: "!toggleredeem",
        required_role: UserRole::Moderator,
        handler: |msg, client, channel, _api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_toggle_redeem(msg, client, channel, redeem_manager, storage, user_links, params)),
        description: "Toggles a channel point redeem on or off",
    },
    Command {
        name: "!setofflineredeem",
        required_role: UserRole::Moderator,
        handler: |msg, client, channel, _api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_set_offline_redeem(msg, client, channel, redeem_manager, storage, user_links, params)),
        description: "Sets whether a redeem is available in offline chat",
    },
    Command {
        name: "!verify",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, _api_client, _world_info, _cooldowns, _redeem_manager, _role_cache, _storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::verify::handle_verify(msg, client, channel, user_links, params)),
        description: "Verifies and links your Twitch account to your Discord account",
    },
    Command {
        name: "!discord",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, api_client, _world_info, _cooldowns, _redeem_manager, _role_cache, storage, user_links, _params, _config, _vrchat_client, ai_client, _is_stream_online| {
            Box::pin(commands::discord::handle_discord(msg, client, channel, api_client, storage, user_links, ai_client))
        },
        description: "Provides a link to join our Discord community and sends an announcement",
    },
    Command {
        name: "!vrc",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, api_client, _world_info, _cooldowns, _redeem_manager, _role_cache, storage, user_links, _params, _config, _vrchat_client, ai_client, _is_stream_online| {
            Box::pin(commands::vrc::handle_discord(msg, client, channel, api_client, storage, user_links, ai_client))
        },
        description: "Provides a link to join our VRChat community and sends an announcement",
    },
];

pub async fn execute_command(
    command: &Command,
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    api_client: &Arc<TwitchAPIClient>,
    world_info: &Arc<Mutex<Option<World>>>,
    cooldowns: &Arc<Mutex<commands::ShoutoutCooldown>>,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    role_cache: &Arc<RwLock<RoleCache>>,
    storage: &Arc<RwLock<StorageClient>>,
    user_links: &Arc<UserLinks>,
    params: &[&str],
    config: &Arc<RwLock<Config>>,
    vrchat_manager: &Arc<VRChatManager>,
    ai_client: &Option<Arc<AIClient>>,
    is_stream_online: bool
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    (command.handler)(
        msg,
        client,
        channel,
        api_client,
        world_info,
        cooldowns,
        redeem_manager,
        role_cache,
        storage,
        user_links,
        params,
        config,
        vrchat_manager,
        ai_client,
        is_stream_online
    ).await
}