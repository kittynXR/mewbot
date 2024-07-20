use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;

pub async fn handle(
    event: &Value,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
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

        irc_client.say(channel.to_string(), message).await?;
    }

    Ok(())
}