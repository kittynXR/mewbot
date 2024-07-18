use serde_json::Value;
use super::events;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;

pub async fn handle_message(
    message: &str,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Received EventSub message: {}", message);
    let parsed: Value = serde_json::from_str(message)?;

    if let Some(event_type) = parsed["metadata"]["subscription_type"].as_str() {
        match event_type {
            "channel.update" => events::channel_update::handle(&parsed, irc_client, channel).await?,
            _ => println!("Unhandled event type: {}", event_type),
        }
    }

    Ok(())
}