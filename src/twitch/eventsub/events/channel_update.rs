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
        if let (Some(title), Some(category_name)) = (payload["title"].as_str(), payload["category_name"].as_str()) {
            println!("Channel update event: Title: '{}', Category: '{}'", title, category_name);

            let response = format!("Channel updated! Category: {} Title: {}", category_name, title);

            irc_client.say(channel.to_string(), response).await?;
        }
    }

    Ok(())
}