use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::TwitchManager;

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let user_name = payload["user_name"].as_str().unwrap_or("Unknown");
        let message = payload["message"].as_str().unwrap_or("");
        let cumulative_months = payload["cumulative_months"].as_u64().unwrap_or(0);

        let response = format!("Thank you {} for {} months of support! They said: {}", user_name, cumulative_months, message);
        twitch_manager.send_message_as_bot(channel, response.as_str()).await?;
    }

    Ok(())
}