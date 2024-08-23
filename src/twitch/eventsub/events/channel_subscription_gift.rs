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
        let user_name = payload["user_name"].as_str().unwrap_or("Anonymous");
        let total = payload["total"].as_u64().unwrap_or(0);
        let tier = payload["tier"].as_str().unwrap_or("1000");
        let cumulative_total = payload["cumulative_total"].as_u64().unwrap_or(0);

        let tier_name = match tier {
            "1000" => "Tier 1",
            "2000" => "Tier 2",
            "3000" => "Tier 3",
            _ => "Unknown Tier",
        };

        let message = format!(
            "WOW! {} just gifted {} {} subscriptions! They've gifted a total of {} subs in the channel!",
            user_name, total, tier_name, cumulative_total
        );

        twitch_manager.send_message_as_bot(channel, message.as_str()).await?;
    }

    Ok(())
}