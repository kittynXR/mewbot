use log::{info, warn, error};
use crate::twitch::api::TwitchAPIClient;
use serde_json::Value;
use crate::twitch::api::client::TwitchAPIError;
use std::error::Error as StdError;

#[derive(thiserror::Error, Debug)]
pub enum ChannelError {
    #[error("API error: {0}")]
    APIError(#[from] TwitchAPIError),
    #[error("Failed to parse channel information")]
    ParseError,
    #[error("Other error: {0}")]
    Other(Box<dyn StdError + Send + Sync>),
}

impl From<Box<dyn StdError + Send + Sync>> for ChannelError {
    fn from(err: Box<dyn StdError + Send + Sync>) -> Self {
        ChannelError::Other(err)
    }
}

pub async fn get_channel_game(user_id: &str, api_client: &TwitchAPIClient) -> Result<String, ChannelError> {
    info!("Fetching game for channel with user ID: {}", user_id);

    let channel_info = api_client.get_channel_information(user_id).await?;

    if let Some(data) = channel_info["data"].as_array() {
        if let Some(channel) = data.first() {
            if let Some(game_name) = channel["game_name"].as_str() {
                info!("Game for channel {}: {}", user_id, game_name);
                return Ok(game_name.to_string());
            }
        }
    }

    warn!("Unable to determine game for channel {}", user_id);
    Ok("Unknown game".to_string())
}

#[derive(Debug, Clone)]
pub struct Clip {
    pub title: String,
    pub url: String,
}

impl TryFrom<&Value> for Clip {
    type Error = ChannelError;

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        let title = value["title"].as_str().ok_or(ChannelError::ParseError)?;
        let url = value["url"].as_str().ok_or(ChannelError::ParseError)?;

        Ok(Clip {
            title: title.to_string(),
            url: url.to_string(),
        })
    }
}

pub async fn get_top_clips(api_client: &TwitchAPIClient, broadcaster_id: &str, limit: u32) -> Result<Vec<Clip>, ChannelError> {
    info!("Fetching top {} clips for broadcaster ID: {}", limit, broadcaster_id);

    let clips = api_client.get_top_clips(broadcaster_id, limit).await?;

    info!("Successfully fetched {} clips for broadcaster ID: {}", clips.len(), broadcaster_id);

    Ok(clips)
}

pub async fn update_channel_title(
    api_client: &TwitchAPIClient,
    broadcaster_id: &str,
    title: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let response = api_client.client
        .patch("https://api.twitch.tv/helix/channels")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .query(&[("broadcaster_id", broadcaster_id)])
        .json(&serde_json::json!({
            "title": title
        }))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to update channel title. Status: {}, Error: {}",
                           status, error_text).into());
    }

    Ok(())
}

pub async fn update_channel_category(
    api_client: &TwitchAPIClient,
    broadcaster_id: &str,
    game_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let response = api_client.client
        .patch("https://api.twitch.tv/helix/channels")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .query(&[("broadcaster_id", broadcaster_id)])
        .json(&serde_json::json!({
            "game_id": game_id
        }))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to update channel category. Status: {}, Error: {}",
                           status, error_text).into());
    }

    Ok(())
}

pub async fn update_content_classification_labels(
    api_client: &TwitchAPIClient,
    broadcaster_id: &str,
    labels: Vec<(String, bool)>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let ccls = labels.into_iter().map(|(id, is_enabled)| {
        serde_json::json!({
            "id": id,
            "is_enabled": is_enabled
        })
    }).collect::<Vec<_>>();

    let response = api_client.client
        .patch("https://api.twitch.tv/helix/channels")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .query(&[("broadcaster_id", broadcaster_id)])
        .json(&serde_json::json!({
            "content_classification_labels": ccls
        }))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to update CCLs. Status: {}, Error: {}",
                           status, error_text).into());
    }

    Ok(())
}

pub async fn start_commercial(
    api_client: &TwitchAPIClient,
    broadcaster_id: &str,
    length: i32,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let response = api_client.client
        .post("https://api.twitch.tv/helix/channels/commercial")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "broadcaster_id": broadcaster_id,
            "length": length
        }))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to start commercial. Status: {}, Error: {}",
                           status, error_text).into());
    }

    Ok(())
}

pub async fn search_category(
    api_client: &TwitchAPIClient,
    query: &str,
) -> Result<Option<(String, String)>, Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let response = api_client.client
        .get("https://api.twitch.tv/helix/search/categories")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .query(&[("query", query)])
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to search categories. Status: {}, Error: {}",
                           status, error_text).into());
    }

    let body: serde_json::Value = response.json().await?;

    // Get the first (best) match
    if let Some(categories) = body["data"].as_array() {
        if let Some(first_match) = categories.first() {
            let game_id = first_match["id"].as_str()
                .ok_or("Game ID not found in response")?;
            let game_name = first_match["name"].as_str()
                .ok_or("Game name not found in response")?;
            return Ok(Some((game_id.to_string(), game_name.to_string())));
        }
    }

    Ok(None)
}