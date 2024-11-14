// src/twitch/events/stream_online.rs
use serde_json::Value;
use std::sync::Arc;
use log::{info, error};
use crate::twitch::TwitchManager;
use serenity::model::id::ChannelId;

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let broadcaster_user_name = payload["broadcaster_user_name"].as_str().unwrap_or("Unknown");
        let started_at = payload["started_at"].as_str().unwrap_or("Unknown time");
        let game_name = payload["category_name"].as_str();
        let title = payload["title"].as_str();
        let thumbnail_url = payload["thumbnail_url"].as_str();

        let message = format!("{} has gone live! Stream started at {}. Come join the fun!",
                              broadcaster_user_name, started_at);
        twitch_manager.send_message_as_bot(channel, &message).await?;

        // Send Discord announcement if configured
        if let Some(discord_client) = &twitch_manager.discord_client {
            if let Some(announcement_channel_id) = &twitch_manager.config.discord_announcement_channel_id {
                if let Ok(channel_id) = announcement_channel_id.parse::<u64>() {
                    let http = discord_client.get_http().await;
                    match crate::discord::announcements::send_stream_announcement(
                        &http,
                        ChannelId::new(channel_id),
                        broadcaster_user_name,
                        started_at,
                        game_name,
                        title,
                        thumbnail_url,
                    ).await {
                        Ok(_) => info!("Discord announcement sent successfully"),
                        Err(e) => error!("Failed to send Discord announcement: {}", e),
                    }
                }
            }
        }

        let game_name = game_name.unwrap_or("").to_string();
        twitch_manager.stream_state_machine.set_stream_live(game_name).await?;

        info!("Stream is now live");
    }

    Ok(())
}