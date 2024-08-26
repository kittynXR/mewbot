use crate::twitch::roles::UserRole;
use crate::vrchat::models::World;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use twitch_irc::message::PrivmsgMessage;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::future::Future;
use std::pin::Pin;
use crate::ai::AIClient;
use crate::twitch::redeems::RedeemManager;
use crate::config::Config;
use crate::twitch::irc::TwitchBotClient;
use crate::vrchat::VRChatManager;
use crate::twitch::manager::TwitchManager;
use super::commands;

pub struct Command {
    pub name: &'static str,
    pub required_role: UserRole,
    pub handler: for<'a> fn(
        &'a PrivmsgMessage,
        &'a Arc<TwitchBotClient>,
        &'a str,
        &'a Arc<TwitchManager>,
        &'a Arc<Mutex<Option<World>>>,
        &'a Arc<Mutex<commands::ShoutoutCooldown>>,
        &'a Arc<RwLock<RedeemManager>>,
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
        name: "!calc",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, _twitch_manager, _world_info, _cooldowns, _redeem_manager, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online| {
            Box::pin(commands::calc::handle_calc(msg, client, channel, storage, user_links, params))
        },
        description: "Calculates a mathematical expression",
    },
    Command {
        name: "!world",
        required_role: UserRole::Subscriber,
        handler: |msg, client, channel, twitch_manager, world_info, _cooldowns, _redeem_manager, storage, user_links, _params, _config, vrchat_client, _ai_client, is_stream_online|
            Box::pin(commands::handle_world(msg, client, channel, world_info, storage, user_links, vrchat_client, is_stream_online)),
        description: "Shows information about the current VRChat world",
    },
    Command {
        name: "!uptime",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, twitch_manager, _world_info, _cooldowns, _redeem_manager, storage, user_links, _params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_uptime(msg, client, channel, twitch_manager, storage, user_links)),
        description: "Shows how long the stream has been live",
    },
    Command {
        name: "!ping",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, _twitch_manager, _world_info, _cooldowns, _redeem_manager, storage, user_links, _params, _config, _vrchat_client, _ai_client, _is_stream_online|
            Box::pin(commands::handle_ping(msg, client, channel, storage, user_links)),
        description: "Responds with Pong!",
    },
    Command {
        name: "!so",
        required_role: UserRole::Subscriber,
        handler: |msg, client, channel, twitch_manager, _world_info, cooldowns, redeem_manager, storage, user_links, params, _config, _vrchat_client, ai_client, _is_stream_online|
            Box::pin(commands::handle_shoutout(msg, client, channel, twitch_manager, cooldowns, params, redeem_manager, storage, user_links, ai_client)),
        description: "Gives a shoutout to another streamer",
    },
    // Command {
    //     name: "!clearcache",
    //     required_role: UserRole::Broadcaster,
    //     handler: |_msg, client, channel, _api_client, _world_info, _cooldowns, _redeem_manager, role_cache, _storage, _user_links, _params, _config, _vrchat_client, _ai_client, _is_stream_online| Box::pin(async move {
    //         role_cache.write().await.clear();
    //         client.send_message(channel, "Role cache has been cleared.").await?;
    //         Ok(())
    //     }),
    //     description: "Clears the role cache",
    // },
    // Command {
    //     name: "!complete",
    //     required_role: UserRole::Subscriber,
    //     handler: |msg, client, channel, _api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
    //         Box::pin(commands::handle_complete_redemption(msg, client, channel, redeem_manager, storage, user_links, params)),
    //     description: "Marks a queued redemption as complete",
    // },
    // Command {
    //     name: "!add_redeem",
    //     required_role: UserRole::Moderator,
    //     handler: |msg, client, channel, api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
    //         Box::pin(commands::handle_add_redeem(msg, client, channel, api_client, redeem_manager, storage, user_links, params)),
    //     description: "Adds a new channel point redemption. Usage: !add_redeem \"<title>\" <cost> <action_type> <cooldown> \"<prompt>\" [queued] [announce]",
    // },
    // Command {
    //     name: "!setactivegames",
    //     required_role: UserRole::Moderator,
    //     handler: |msg, client, channel, _api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
    //         Box::pin(commands::handle_set_active_games(msg, client, channel, redeem_manager, storage, user_links, params)),
    //     description: "Sets the games for which a redeem is active",
    // },
    // Command {
    //     name: "!toggleredeem",
    //     required_role: UserRole::Moderator,
    //     handler: |msg, client, channel, _api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
    //         Box::pin(commands::handle_toggle_redeem(msg, client, channel, redeem_manager, storage, user_links, params)),
    //     description: "Toggles a channel point redeem on or off",
    // },
    // Command {
    //     name: "!setofflineredeem",
    //     required_role: UserRole::Moderator,
    //     handler: |msg, client, channel, _api_client, _world_info, _cooldowns, redeem_manager, _role_cache, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
    //         Box::pin(commands::handle_set_offline_redeem(msg, client, channel, redeem_manager, storage, user_links, params)),
    //     description: "Sets whether a redeem is available in offline chat",
    // },
    // Command {
    //     name: "!verify",
    //     required_role: UserRole::Viewer,
    //     handler: |msg, client, channel, _api_client, _world_info, _cooldowns, _redeem_manager, _role_cache, _storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online|
    //         Box::pin(commands::verify::handle_verify(msg, client, channel, user_links, params)),
    //     description: "Verifies and links your Twitch account to your Discord account",
    // },
    Command {
        name: "!discord",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, twitch_manager, world_info, _cooldowns, _redeem_manager, storage, user_links, _params, _config, vrchat_client, ai_client, is_stream_online| {
            Box::pin(commands::discord::handle_discord(msg, client, channel, twitch_manager, storage, user_links, ai_client))
        },
        description: "Provides a link to join our Discord community and sends an announcement",
    },
    Command {
        name: "!vrc",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, twitch_manager, world_info, _cooldowns, _redeem_manager, storage, user_links, _params, _config, vrchat_client, ai_client, is_stream_online| {
            Box::pin(commands::vrc::handle_discord(msg, client, channel, twitch_manager, storage, user_links, ai_client))
        },
        description: "Provides a link to join our VRChat community and sends an announcement",
    },
    Command {
        name: "!followers",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, twitch_manager, _world_info, _cooldowns, _redeem_manager, storage, user_links, _params, _config, _vrchat_client, _ai_client, _is_stream_online| {
            Box::pin(commands::followers::handle_followers(msg, client, channel, twitch_manager, storage, user_links))
        },
        description: "Shows the current number of followers for the channel",
    },
    Command {
        name: "!followage",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, twitch_manager, _world_info, _cooldowns, _redeem_manager, storage, user_links, params, _config, _vrchat_client, _ai_client, _is_stream_online| {
            Box::pin(commands::followers::handle_followage(msg, client, channel, twitch_manager, storage, user_links, params))
        },
        description: "Shows how long a user has been following the channel",
    },
    Command {
        name: "!isitfriday",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, twitch_manager, _world_info, _cooldowns, _redeem_manager, storage, user_links, _params, _config, _vrchat_client, ai_client, _is_stream_online| {
            Box::pin(commands::fun_commands::handle_isitfriday(msg, client, channel, twitch_manager, storage, user_links, ai_client))
        },
        description: "Check if it's Friday and get a fun message",
    },
    Command {
        name: "!xmas",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, twitch_manager, _world_info, _cooldowns, _redeem_manager, storage, user_links, _params, _config, _vrchat_client, ai_client, _is_stream_online| {
            Box::pin(commands::fun_commands::handle_xmas(msg, client, channel, twitch_manager, storage, user_links, ai_client))
        },
        description: "Find out how many days until Christmas",
    },
];

pub async fn execute_command(
    command: &Command,
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
    world_info: &Arc<Mutex<Option<World>>>,
    cooldowns: &Arc<Mutex<commands::ShoutoutCooldown>>,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
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
        twitch_manager,
        world_info,
        cooldowns,
        redeem_manager,
        storage,
        user_links,
        params,
        config,
        vrchat_manager,
        ai_client,
        is_stream_online
    ).await
}