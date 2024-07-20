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

        irc_client.say(channel.to_string(), message).await?;
    }

    Ok(())
}