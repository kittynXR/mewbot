use serde_json::Value;
use std::sync::Arc;
use log::info;
use crate::twitch::TwitchManager;

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        if let (Some(title), Some(category_name)) = (payload["title"].as_str(), payload["category_name"].as_str()) {
            info!("Channel update event: Title: '{}', Category: '{}'", title, category_name);

            let response = format!("Channel updated! Category: {} Title: {}", category_name, title);

            twitch_manager.send_message_as_bot(channel, response.as_str()).await?;

            twitch_manager.handle_stream_update(category_name.to_string()).await?;
        }
    }

    Ok(())
}