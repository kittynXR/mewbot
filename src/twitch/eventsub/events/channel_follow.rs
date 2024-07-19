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
        if let Some(user_name) = payload["user_name"].as_str() {
            println!("New follower: {}", user_name);

            let response = format!("Thank you for following, {}! Welcome to the community! mao mao", user_name);

            irc_client.say(channel.to_string(), response).await?;
        }
    }

    Ok(())
}