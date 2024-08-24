use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use log::{error, info};
use tokio::sync::RwLock;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::redeems::RedeemManager;
use crate::twitch::TwitchManager;

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        if let (Some(title), Some(category_name)) = (payload["title"].as_str(), payload["category_name"].as_str()) {
            info!("Channel update event: Title: '{}', Category: '{}'", title, category_name);

            let response = format!("Channel updated! Category: {} Title: {}", category_name, title);

            twitch_manager.send_message_as_bot(channel, response.as_str()).await?;

            // let redeem_manager = twitch_manager.get_redeem_manager();
            // Update stream status with the new game
            // let mut manager = redeem_manager.write().await;
            // if let Err(e) = manager.update_stream_status(category_name.to_string()).await {
            //     error!("Failed to update stream status: {}", e);
            // }
        }
    }

    Ok(())
}