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
        let message = payload["message"].as_str().unwrap_or("");
        let cumulative_months = payload["cumulative_months"].as_u64().unwrap_or(0);

        let response = format!("Thank you {} for {} months of support! They said: {}", user_name, cumulative_months, message);
        irc_client.say(channel.to_string(), response).await?;
    }

    Ok(())
}