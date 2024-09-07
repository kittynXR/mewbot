use serde_json::Value;
use std::sync::Arc;
use crate::twitch::TwitchManager;

pub async fn handle_shoutout_create(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let to_broadcaster_user_name = payload["to_broadcaster_user_name"].as_str().unwrap_or("Unknown");
        let to_broadcaster_user_id = payload["to_broadcaster_user_id"].as_str().unwrap_or("Unknown");

        let message = format!("Hey, you should go check out {}! Click the heart at the top of the chatbox ~ it's easy! luv luv", to_broadcaster_user_name);
        twitch_manager.send_message_as_bot(channel, message.as_str()).await?;

        // Handle the successful shoutout in TwitchManager
        twitch_manager.handle_shoutout_create_event(to_broadcaster_user_id).await;
    }

    Ok(())
}

pub async fn handle_shoutout_receive(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let from_broadcaster_user_name = payload["from_broadcaster_user_name"].as_str().unwrap_or("Unknown");
        let viewer_count = payload["viewer_count"].as_u64().unwrap_or(0);

        let message = format!("We just received a shoutout from {} with {} viewers! Thank you for the support!", from_broadcaster_user_name, viewer_count);
        twitch_manager.send_message_as_bot(channel, message.as_str()).await?;
    }

    Ok(())
}