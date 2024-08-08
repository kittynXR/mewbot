use lazy_static::lazy_static;
use crate::vrchat::models::World;
use std::sync::Arc;
use crate::config::Config;
use tokio::sync::{Mutex, RwLock};
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::PrivmsgMessage;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::roles::{get_user_role, UserRole};
use super::command_system::COMMANDS;
use super::commands;
use crate::twitch::redeems::RedeemManager;
use crate::osc::vrchat::VRChatOSC;
use crate::storage::StorageClient;
use crate::twitch::role_cache::RoleCache;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::discord::UserLinks;
use crate::logging::Logger;

lazy_static! {
    static ref SHOUTOUT_COOLDOWNS: Arc<Mutex<commands::ShoutoutCooldown>> = Arc::new(Mutex::new(commands::ShoutoutCooldown::new()));
    static ref VRCHAT_OSC: Mutex<Option<VRChatOSC>> = Mutex::new(None);
    static ref BROADCASTER_ID: AtomicU64 = AtomicU64::new(0);
}

pub async fn handle_twitch_message(
    msg: &PrivmsgMessage,
    client: Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    world_info: Arc<Mutex<Option<World>>>,
    api_client: Arc<Arc<TwitchAPIClient>>,
    config: Arc<RwLock<Config>>,
    redemption_manager: Arc<RwLock<RedeemManager>>,
    vrchat_osc: Option<Arc<VRChatOSC>>,
    storage: Arc<RwLock<StorageClient>>,
    role_cache: Arc<RwLock<RoleCache>>,
    user_links: Arc<UserLinks>,
    logger: Arc<Logger>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Handling message: {:?}", msg.message_text);

    let channel_id = if BROADCASTER_ID.load(Ordering::Relaxed) == 0 {
        let id = api_client.get_broadcaster_id().await?;
        BROADCASTER_ID.store(id.parse::<u64>().unwrap_or(0), Ordering::Relaxed);
        id
    } else {
        BROADCASTER_ID.load(Ordering::Relaxed).to_string()
    };
    println!("Channel ID: {}", channel_id);

    let user_role = match get_user_role(&msg.sender.id, &channel_id, &api_client, &storage, &role_cache).await {
        Ok(role) => role,
        Err(e) => {
            println!("Error getting user role: {:?}. Defaulting to Viewer.", e);
            UserRole::Viewer
        }
    };
    println!("User role for {}: {:?}", msg.sender.name, user_role);

    // Clean the message: remove invisible characters and trim
    let cleaned_message = msg.message_text
        .chars()
        .filter(|&c| c.is_ascii_graphic() || c.is_ascii_whitespace())
        .collect::<String>()
        .trim()
        .to_string();

    println!("Cleaned message: {}", cleaned_message);

    // Create a lowercase version of the cleaned message for command matching
    let lowercase_message = cleaned_message.to_lowercase();

    // Split the lowercase message into command and parameters
    let mut parts = lowercase_message.split_whitespace();
    let command = parts.next();
    let params: Vec<&str> = parts.collect();

    if let Some(cmd) = command {
        println!("Received command: {}", cmd);
        if let Some(command) = COMMANDS.iter().find(|c| c.name == cmd) {
            println!("Matched command: {}", command.name);
            if user_role >= command.required_role {
                println!("User has sufficient permissions. Executing command.");
                let result = (command.handler)(
                    msg,
                    &client,
                    &msg.channel_login,
                    &api_client,
                    &world_info,
                    &SHOUTOUT_COOLDOWNS,
                    &redemption_manager,
                    &role_cache,
                    &storage,
                    &user_links,
                    &params,
                    &config,
                    &logger
                ).await;
                println!("Command execution result: {:?}", result);
                return result;
            } else {
                println!("User does not have sufficient permissions.");
                let response = format!("@{}, this command is only available to {:?}s and above.", msg.sender.name, command.required_role);
                println!("Sending response: {}", response);
                client.say(msg.channel_login.clone(), response).await?;
                return Ok(());
            }
        } else {
            println!("Command not found in COMMANDS list");
        }
    }

    println!("Message processing complete");
    Ok(())
}