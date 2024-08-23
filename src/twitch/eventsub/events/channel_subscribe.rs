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
        let tier = payload["tier"].as_str().unwrap_or("1000");
        let is_gift = payload["is_gift"].as_bool().unwrap_or(false);

        let tier_name = match tier {
            "1000" => "Tier 1",
            "2000" => "Tier 2",
            "3000" => "Tier 3",
            _ => "Unknown Tier",
        };

        let message = if is_gift {
            format!("{} received a gifted {} subscription! Thank you to the generous gifter!", user_name, tier_name)
        } else {
            format!("Thank you {} for subscribing with a {} subscription!", user_name, tier_name)
        };

        twitch_manager.send_message_as_bot(channel, message.as_str()).await?;
    }

    Ok(())
}