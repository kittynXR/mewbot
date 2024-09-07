use serde_json::Value;
use std::sync::Arc;
use log::{info};
use crate::twitch::manager::TwitchManager;

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let broadcaster_user_name = payload["broadcaster_user_name"].as_str().unwrap_or("Unknown");

        let message = format!("{} ended stream!  mao  Stay amazing and cute! mao", broadcaster_user_name);
        twitch_manager.send_message_as_bot(channel, message.as_str()).await?;

        let message = format!("bark bark bark bark bark bark bark bark bark");
        twitch_manager.send_message_as_bot(channel, message.as_str()).await?;

        // Update stream status using StreamStatusManager
        twitch_manager.set_stream_live(false).await;

        if let Some(redeem_manager) = twitch_manager.get_redeem_manager().write().await.as_mut() {
            redeem_manager.handle_stream_offline().await?;
        }

        info!("Stream is now offline");
    }

    Ok(())
}