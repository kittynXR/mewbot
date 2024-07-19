use crate::vrchat::models::World;
use std::sync::Arc;
use tokio::sync::Mutex;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::PrivmsgMessage;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::irc::commands;

pub async fn handle_twitch_message(
    msg: PrivmsgMessage,
    client: Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    world_info: Arc<Mutex<Option<World>>>,
    api_client: Arc<TwitchAPIClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Handling Twitch message: {:?}", msg);

    // Clean the message: remove invisible characters and trim
    let cleaned_message = msg.message_text
        .chars()
        .filter(|&c| c.is_ascii_graphic() || c.is_ascii_whitespace())
        .collect::<String>()
        .trim()
        .to_lowercase();

    println!("Cleaned message: '{}'", cleaned_message);

    // Split the message into command and parameters
    let mut parts = cleaned_message.split_whitespace();
    let command = parts.next();
 //   let params: Vec<&str> = parts.collect();

    match command {
        Some("!world") => {
            commands::handle_world(&msg, &client, &msg.channel_login, &world_info).await?;
        },
        Some("!uptime") => {
            commands::handle_uptime(&msg, &client, &msg.channel_login, &api_client).await?;
        },
        Some("!hello") => {
            commands::handle_hello(&msg, &client, &msg.channel_login).await?;
        },
        Some("!ping") => {
            commands::handle_ping(&msg, &client, &msg.channel_login).await?;
        },
        Some(cmd) => {
            println!("Unknown command: {}", cmd);
            // Optionally, respond to unknown commands
        },
        None => {
            println!("Empty message received.");
        }
    }

    Ok(())
}