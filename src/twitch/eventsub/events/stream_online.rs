use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use log::error;
use crate::twitch::redeems::RedeemManager;
use tokio::sync::RwLock;

// In stream_online.rs

pub async fn handle(
    event: &Value,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let broadcaster_user_name = payload["broadcaster_user_name"].as_str().unwrap_or("Unknown");
        let started_at = payload["started_at"].as_str().unwrap_or("Unknown time");

        let message = format!("{} has gone live! Stream started at {}. Come join the fun!", broadcaster_user_name, started_at);
        irc_client.say(channel.to_string(), message).await?;

        // Update stream status
        let mut manager = redeem_manager.write().await;
        if let Err(e) = manager.set_stream_live(true).await {
            error!("Failed to set stream as live: {}", e);
        }
    }

    Ok(())
}