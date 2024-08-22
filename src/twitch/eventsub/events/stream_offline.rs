use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use log::{error, info};
use crate::twitch::manager::TwitchManager;

pub async fn handle(
    event: &Value,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let broadcaster_user_name = payload["broadcaster_user_name"].as_str().unwrap_or("Unknown");

        let message = format!("{} ended stream!  mao  Stay amazing and cute! mao", broadcaster_user_name);
        irc_client.say(channel.to_string(), message).await?;

        let message = format!("bark bark bark bark bark bark bark bark bark");
        irc_client.say(channel.to_string(), message).await?;

        // Update stream status using TwitchManager
        twitch_manager.set_stream_live(false).await;
        info!("Stream is now offline");
    }

    Ok(())
}