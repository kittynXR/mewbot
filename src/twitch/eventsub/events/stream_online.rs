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
        let started_at = payload["started_at"].as_str().unwrap_or("Unknown time");

        let message = format!("{} has gone live! Stream started at {}. Come join the fun!", broadcaster_user_name, started_at);
        twitch_manager.send_message_as_bot(channel, message.as_str()).await?;

        // Update stream status using TwitchManager
        twitch_manager.set_stream_live(true).await;

        let game_name = payload["category_name"].as_str().unwrap_or("").to_string();

        if let Some(redeem_manager) = twitch_manager.get_redeem_manager().write().await.as_mut() {
            redeem_manager.handle_stream_online(game_name).await?;
        }

        info!("Stream is now live");
    }

    Ok(())
}