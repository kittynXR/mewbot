// src/twitch/irc/commands/verify.rs

use crate::storage::StorageClient;
use crate::discord::UserLinks;
use twitch_irc::message::PrivmsgMessage;
use crate::twitch::irc::TwitchBotClient;
use std::sync::Arc;
use log::{info, warn};
use tokio::sync::RwLock;

pub async fn handle_verify(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    user_links: &Arc<UserLinks>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if params.len() != 1 {
        client.send_message(channel, "Usage: !verify <code>").await?;
        return Ok(());
    }

    let code = params[0].parse::<u32>().map_err(|_| "Invalid verification code")?;
    let twitch_username = msg.sender.name.clone();

    info!("Attempting to verify Twitch user: {} with code: {}", twitch_username, code);

    match user_links.verify_and_link(&twitch_username, code).await {
        Ok(discord_id) => {
            info!("Successfully linked Twitch user: {} to Discord ID: {}", twitch_username, discord_id);
            client.send_message(channel, &format!("@{}, your Twitch account has been successfully verified and linked to your Discord account!", twitch_username)).await?;
        },
        Err(e) => {
            warn!("Verification failed for Twitch user: {}. Error: {}", twitch_username, e);
            client.send_message(channel, &format!("@{}, verification failed: {}. Please use the /linktwitch command in Discord to get a new verification code.", twitch_username, e)).await?;
        }
    }

    Ok(())
}
