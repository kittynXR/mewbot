use serde_json::Value;
use std::sync::Arc;
use log::{info, error};
use crate::twitch::TwitchManager;
use serenity::model::id::ChannelId;

async fn generate_stream_description(
    ai_client: &Arc<crate::ai::AIClient>,
    broadcaster_name: &str,
    title: &str,
    game_name: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let prompt = format!(
        "Generate a short, exciting message (max 100 characters) about {}'s stream. \
        They're playing {} with the title: '{}'. Make it engaging and fun!",
        broadcaster_name, game_name, title
    );

    ai_client.generate_response_without_history(&prompt).await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
}

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

                    // Get broadcaster profile image
                    let api_client = twitch_manager.get_api_client();
                    let user_info = api_client.get_user_info(broadcaster_user_name).await?;
                    let profile_image_url = user_info["data"][0]["profile_image_url"].as_str();

                    // Generate AI message if we have an AI client
                    let ai_message = if let Some(ai_client) = &twitch_manager.ai_client {
                        if let (Some(t), Some(g)) = (title, game_name) {
                            match generate_stream_description(ai_client, broadcaster_user_name, t, g).await {
                                Ok(msg) => Some(msg),
                                Err(e) => {
                                    error!("Failed to generate AI description: {}", e);
                                    None
                                }
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    match crate::discord::announcements::send_stream_announcement(
                        &http,
                        ChannelId::new(channel_id),
                        broadcaster_user_name,
                        started_at,
                        game_name,
                        title,
                        thumbnail_url,
                        profile_image_url,
                        ai_message.as_deref(),
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