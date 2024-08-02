use serde_json::Value;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;

pub async fn handle_shoutout_create(
    event: &Value,
    irc_client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let from_broadcaster_user_name = payload["from_broadcaster_user_name"].as_str().unwrap_or("Unknown");
        let to_broadcaster_user_name = payload["to_broadcaster_user_name"].as_str().unwrap_or("Unknown");

        let message = format!("Hey, you should go check out {}! Click the heart at the top of the chatbox ~ it's easy! luv luv", to_broadcaster_user_name);
        irc_client.say(channel.to_string(), message).await?;
    }

    Ok(())
}

pub async fn handle_shoutout_receive(
    event: &Value,
    irc_client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let from_broadcaster_user_name = payload["from_broadcaster_user_name"].as_str().unwrap_or("Unknown");
        let viewer_count = payload["viewer_count"].as_u64().unwrap_or(0);

        let message = format!("We just received a shoutout from {} with {} viewers! Thank you for the support!", from_broadcaster_user_name, viewer_count);
        irc_client.say(channel.to_string(), message).await?;
    }

    Ok(())
}