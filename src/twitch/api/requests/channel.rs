use crate::twitch::api::TwitchAPIClient;
use serde_json::Value;

pub async fn get_channel_game(user_id: &str, api_client: &TwitchAPIClient) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let response = api_client.client
        .get(&format!("https://api.twitch.tv/helix/channels?broadcaster_id={}", user_id))
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to get channel info. Status: {}", response.status()).into());
    }

    let body: Value = response.json().await?;

    if let Some(data) = body["data"].as_array() {
        if let Some(channel) = data.first() {
            if let Some(game_name) = channel["game_name"].as_str() {
                return Ok(game_name.to_string());
            }
        }
    }

    Ok("Unknown game".to_string())
}

pub async fn get_channel_information(api_client: &TwitchAPIClient, broadcaster_id: &str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let response = api_client.client
        .get(&format!("https://api.twitch.tv/helix/channels?broadcaster_id={}", broadcaster_id))
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to get channel information. Status: {}", response.status()).into());
    }

    let body: serde_json::Value = response.json().await?;
    Ok(body)
}