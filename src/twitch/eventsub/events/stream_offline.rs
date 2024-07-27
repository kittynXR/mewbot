use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use crate::twitch::redeems::RedeemManager;
use tokio::sync::RwLock;


// In stream_offline.rs
pub async fn handle(
    event: &Value,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let broadcaster_user_name = payload["broadcaster_user_name"].as_str().unwrap_or("Unknown");

        let message = format!("{} has ended the stream. Thanks for watching!", broadcaster_user_name);
        irc_client.say(channel.to_string(), message).await?;

        // Update stream status
        redeem_manager.write().await.update_stream_status(false, "".to_string()).await;
    }

    Ok(())
}