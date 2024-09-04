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