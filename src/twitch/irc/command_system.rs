use crate::twitch::roles::UserRole;
use crate::vrchat::models::World;
use crate::twitch::api::TwitchAPIClient;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::future::Future;
use std::pin::Pin;
use super::commands;

pub struct Command {
    pub name: &'static str,
    pub required_role: UserRole,
    pub handler: for<'a> fn(&'a PrivmsgMessage, &'a Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>, &'a str, &'a Arc<TwitchAPIClient>, &'a Arc<Mutex<Option<World>>>, &'a Arc<Mutex<commands::ShoutoutCooldown>>, &'a [&'a str]) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>>,
    pub description: &'static str,
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "!world",
        required_role: UserRole::Subscriber,
        handler: |msg, client, channel, _, world_info, _, _| Box::pin(commands::handle_world(msg, client, channel, world_info)),
        description: "Shows information about the current VRChat world",
    },
    Command {
        name: "!uptime",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, api_client, _, _, _| Box::pin(commands::handle_uptime(msg, client, channel, api_client)),
        description: "Shows how long the stream has been live",
    },
    Command {
        name: "!hello",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, _, _, _, _| Box::pin(commands::handle_hello(msg, client, channel)),
        description: "Greets the user",
    },
    Command {
        name: "!ping",
        required_role: UserRole::Viewer,
        handler: |msg, client, channel, _, _, _, _| Box::pin(commands::handle_ping(msg, client, channel)),
        description: "Responds with Pong!",
    },
    Command {
        name: "!so",
        required_role: UserRole::Moderator,
        handler: |msg, client, channel, api_client, _, cooldowns, params| Box::pin(commands::handle_shoutout(msg, client, channel, api_client, cooldowns, params)),
        description: "Gives a shoutout to another streamer",
    },
];