use crate::twitch::api::TwitchAPIClient;
use serde_json::Value;

pub async fn get_channel_game(user_id: &str, api_client: &TwitchAPIClient) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let channel_info = api_client.get_channel_information(user_id).await?;

    if let Some(data) = channel_info["data"].as_array() {
        if let Some(channel) = data.first() {
            if let Some(game_name) = channel["game_name"].as_str() {
                return Ok(game_name.to_string());
            }
        }
    }

    Ok("Unknown game".to_string())
}

// The get_channel_information function can be removed as it's now part of TwitchAPIClient

// The get_top_clips function can be removed as it's now part of TwitchAPIClient

#[derive(Debug, Clone)]
pub struct Clip {
    pub title: String,
    pub url: String,
}