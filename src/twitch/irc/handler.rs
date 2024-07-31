
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
use crate::twitch::roles::{UserRole, get_user_role};
use super::command_system::COMMANDS;
use super::commands;
use crate::twitch::redeems::RedeemManager;
use crate::osc::vrchat::VRChatOSC;

lazy_static! {
    static ref SHOUTOUT_COOLDOWNS: Arc<Mutex<commands::ShoutoutCooldown>> = Arc::new(Mutex::new(commands::ShoutoutCooldown::new()));
    static ref VRCHAT_OSC: Mutex<Option<VRChatOSC>> = Mutex::new(None);  // Add this line
}

pub async fn handle_twitch_message(
    msg: &PrivmsgMessage,
    client: Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    world_info: Arc<Mutex<Option<World>>>,
    api_client: Arc<Arc<TwitchAPIClient>>,
    config: Arc<RwLock<Config>>,
    redemption_manager: Arc<RwLock<RedeemManager>>,
    vrchat_osc: Option<Arc<VRChatOSC>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_role = get_user_role(msg);
    println!("Handling Twitch message: {:?}", msg);

    // Clean the message: remove invisible characters and trim
    let cleaned_message = msg.message_text
        .chars()
        .filter(|&c| c.is_ascii_graphic() || c.is_ascii_whitespace())
        .collect::<String>()
        .trim()
        .to_string();

    println!("Cleaned message: '{}'", cleaned_message);

    // Check if the message starts with a backslash and the sender is the broadcaster
    if cleaned_message.starts_with('\\') && user_role == UserRole::Broadcaster {
        if let Some(osc) = vrchat_osc.as_ref() {
            let vrchat_message = &cleaned_message[1..];  // Remove the backslash
            osc.send_chatbox_message(vrchat_message, true, true)?;
            println!("Sent message to VRChat: {}", vrchat_message);
            return Ok(());
        } else {
            println!("VRChatOSC not initialized");
        }
    }

    // Create a lowercase version of the cleaned message for command matching
    let lowercase_message = cleaned_message.to_lowercase();

    // Split the lowercase message into command and parameters
    let mut parts = lowercase_message.split_whitespace();
    let command = parts.next();
    let params: Vec<&str> = parts.collect();

    if let Some(cmd) = command {
        if let Some(command) = COMMANDS.iter().find(|c| c.name == cmd) {
            if user_role >= command.required_role {
                return (command.handler)(msg, &client, &msg.channel_login, &api_client, &world_info, &SHOUTOUT_COOLDOWNS, &redemption_manager.clone(), &params).await
            } else {
                client.say(msg.channel_login.clone(), format!("This command is only available to {:?}s and above.", command.required_role)).await?;
                return Ok(());
            }
        }

        // Handle special commands like !verbose that require access to config
        if cmd == "!verbose" && user_role == UserRole::Broadcaster {
            let mut config = config.write().await;
            config.toggle_verbose_logging()?;
            let status = if config.verbose_logging { "enabled" } else { "disabled" };
            client.say(msg.channel_login.clone(), format!("Verbose logging {}", status)).await?;
        } else {
            println!("Unknown command: {}", cmd);
            // Optionally, respond to unknown commands
        }
    } else {
        println!("Empty message received.");
    }

    Ok(())
}