use crate::vrchat::models::World;
use std::sync::Arc;
use tokio::sync::Mutex;
use twitch_irc::message::ServerMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use crate::twitch::commands::{handle_uptime, handle_world, handle_hello};

pub async fn run(
    client: Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    mut incoming_messages: tokio::sync::mpsc::UnboundedReceiver<ServerMessage>,
    world_info: Arc<Mutex<Option<World>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    while let Some(message) = incoming_messages.recv().await {
        if let ServerMessage::Privmsg(msg) = &message {
            let channel = msg.channel_login.clone();
            let command = msg.message_text.trim().to_lowercase();

            match command.split_whitespace().next() {
                Some("!uptime") => handle_uptime(&message, &client, &channel).await?,
                Some("!world") => handle_world(&message, &client, &channel, &world_info).await?,
                Some("!hello") => handle_hello(&message, &client, &channel).await?,
                _ => {} // Ignore other messages
            }
        }
    }

    Ok(())
}