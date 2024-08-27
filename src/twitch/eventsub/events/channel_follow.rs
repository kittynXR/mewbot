use serde_json::Value;
use std::sync::Arc;
use log::{info, error, warn};
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::TwitchManager;

async fn get_follower_stream_info(api_client: &TwitchAPIClient, user_id: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let stream_info = api_client.get_stream_info(user_id).await?;
    let user_info = api_client.get_user_info_by_id(user_id).await?;

    let mut info = String::new();

    if let Some(user_data) = user_info["data"].as_array().and_then(|arr| arr.first()) {
        if let Some(description) = user_data["description"].as_str() {
            if !description.is_empty() {
                info.push_str(&format!("Channel description: {}. ", description));
            }
        }
    }

    if let Some(stream_data) = stream_info["data"].as_array().and_then(|arr| arr.first()) {
        if let Some(game_name) = stream_data["game_name"].as_str() {
            info.push_str(&format!("Last seen streaming {}. ", game_name));
        }
        if let Some(title) = stream_data["title"].as_str() {
            info.push_str(&format!("Stream title: {}. ", title));
        }
    } else {
        info.push_str("Not currently live. ");
    }

    Ok(info)
}


pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        if let Some(user_name) = payload["user_name"].as_str() {
            if let Some(user_id) = payload["user_id"].as_str() {
                info!("New follower: {}", user_name);

                let api_client = twitch_manager.get_api_client();
                let follower_info = get_follower_stream_info(&api_client, user_id).await?;

                let prompt = format!(
                    "Generate a short, friendly welcome message (1-2 sentences) for a new Twitch follower named {}. Make it warm and inviting, welcoming them to the community. Additional info: {}",
                    user_name, follower_info
                );

                let welcome_message = if let Some(ai_client) = twitch_manager.get_ai_client() {
                    match ai_client.generate_response_without_history(&prompt).await {
                        Ok(ai_response) => ai_response,
                        Err(e) => {
                            error!("Failed to generate AI response: {:?}", e);
                            format!("Welcome to the community, {}! Thanks for following!", user_name)
                        }
                    }
                } else {
                    warn!("AI client not available. Using default welcome message.");
                    format!("Welcome to the community, {}! Thanks for following!", user_name)
                };

                twitch_manager.send_message_as_bot(channel, &welcome_message).await?;
            }
        }
    }

    Ok(())
}
