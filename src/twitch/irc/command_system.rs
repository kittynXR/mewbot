use crate::twitch::roles::UserRole;
use crate::vrchat::models::World;
use crate::twitch::api::TwitchAPIClient;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use chrono::Utc;
use super::commands;
use crate::twitch::redeems::RedeemManager;
use crate::twitch::role_cache::RoleCache;

pub struct Command {
    pub name: &'static str,
    pub required_role: UserRole,
    pub handler: for<'a> fn(&'a PrivmsgMessage, &'a Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>, &'a str, &'a Arc<TwitchAPIClient>, &'a Arc<Mutex<Option<World>>>, &'a Arc<Mutex<commands::ShoutoutCooldown>>, &'a Arc<RwLock<RedeemManager>>, &'a Arc<RwLock<RoleCache>>, &'a [&'a str]) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>>,
    pub description: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "!world",
        required_role: UserRole::Subscriber,
        handler: |msg, client, channel, _, world_info, _, _redemption_manager, _role_cache, _| Box::pin(commands::handle_world(msg, client, channel, world_info)),
        description: "Shows information about the current VRChat world",
    },
    Command {
        name: "!uptime",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, api_client, _, _, _redemption_manager, _role_cache, _| Box::pin(commands::handle_uptime(msg, client, channel, api_client)),
        description: "Shows how long the stream has been live",
    },
    Command {
        name: "!hello",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, _, _, _, _redemption_manager, _role_cache, _| Box::pin(commands::handle_hello(msg, client, channel)),
        description: "Greets the user",
    },
    Command {
        name: "!ping",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, _, _, _, _redemption_manager, _role_cache, _| Box::pin(commands::handle_ping(msg, client, channel)),
        description: "Responds with Pong!",
    },
    Command {
        name: "!so",
        required_role: UserRole::Subscriber,
        handler: |msg, client, channel, api_client, _, cooldowns, _, _, params|
        Box::pin(commands::handle_shoutout(msg, client, channel, api_client, cooldowns, params)),
        description: "Gives a shoutout to another streamer",
    },
    Command {
        name: "!clearcache",
        required_role: UserRole::Broadcaster,
        handler: |_msg, client, channel, _, _, _, _, role_cache, _| Box::pin(async move {
            role_cache.write().await.clear();
            client.say(channel.to_string(), "Role cache has been cleared.".to_string()).await?;
            Ok(())
        }),
        description: "Clears the role cache",
    },
    Command {
        name: "!complete",
        required_role: UserRole::Subscriber,
        handler: |msg, client, channel, _, _, _, redeem_manager, _role_cache, params| Box::pin(commands::handle_complete_redemption(msg, client, channel, redeem_manager, params)),
        description: "Marks a queued redemption as complete",
    },
    Command {
        name: "!add_redeem",
        required_role: UserRole::Moderator, // Or UserRole::Broadcaster, depending on your preference
        handler: |msg, client, channel, api_client, _, _, redeem_manager, _role_cache, params|
        Box::pin(commands::handle_add_redeem(msg, client, channel, api_client, redeem_manager, params)),
        description: "Adds a new channel point redemption. Usage: !add_redeem \"<title>\" <cost> <action_type> <cooldown> \"<prompt>\" [queued] [announce]",
    },
    Command {
        name: "!setactivegames",
        required_role: UserRole::Moderator,
        handler: |msg, client, channel, _, _, _, redeem_manager, _role_cache, params| Box::pin(commands::handle_set_active_games(msg, client, channel, redeem_manager, params)),
        description: "Sets the games for which a redeem is active",
    },
    Command {
        name: "!toggleredeem",
        required_role: UserRole::Moderator,
        handler: |msg, client, channel, _, _, _, redeem_manager, _role_cache, params| Box::pin(commands::handle_toggle_redeem(msg, client, channel, redeem_manager, params)),
        description: "Toggles a channel point redeem on or off",
    },
    Command {
        name: "!setofflineredeem",
        required_role: UserRole::Moderator,
        handler: |msg, client, channel, _, _, _, redeem_manager, _role_cache, params| Box::pin(commands::handle_set_offline_redeem(msg, client, channel, redeem_manager, params)),
        description: "Sets whether a redeem is available in offline chat",
    },
];