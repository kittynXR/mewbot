// twitch/api/client/followers.rs

use crate::twitch::api::TwitchAPIClient;
use serde_json::Value;
use chrono::{DateTime, Utc};
use log::error;

pub async fn get_follower_count(client: &TwitchAPIClient, broadcaster_id: &str) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
    let token = client.get_token().await?;
    let client_id = client.get_client_id().await?;

    let response = client.client
        .get(&format!("https://api.twitch.tv/helix/channels/followers?broadcaster_id={}", broadcaster_id))
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await?;
        error!("Failed to get follower count. Status: {}, Body: {}", status, error_body);
        return Err(format!("Failed to get follower count. Status: {}, Body: {}", status, error_body).into());
    }

    let body: Value = response.json().await?;
    let total_followers = body["total"].as_u64().unwrap_or(0) as u32;

    Ok(total_followers)
}

pub struct FollowerInfo {
    pub user_id: String,
    pub user_login: String,
    pub user_name: String,
    pub followed_at: DateTime<Utc>,
}

pub async fn get_follower_info(client: &TwitchAPIClient, broadcaster_id: &str, user_id: Option<&str>) -> Result<(Vec<FollowerInfo>, u32), Box<dyn std::error::Error + Send + Sync>> {
    let token = client.get_token().await?;
    let client_id = client.get_client_id().await?;

    let mut url = format!("https://api.twitch.tv/helix/channels/followers?broadcaster_id={}", broadcaster_id);
    if let Some(id) = user_id {
        url.push_str(&format!("&user_id={}", id));
    }

    let response = client.client
        .get(&url)
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await?;
        error!("Failed to get follower info. Status: {}, Body: {}", status, error_body);
        return Err(format!("Failed to get follower info. Status: {}, Body: {}", status, error_body).into());
    }

    let body: Value = response.json().await?;
    let total = body["total"].as_u64().unwrap_or(0) as u32;

    let followers: Vec<FollowerInfo> = body["data"].as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|follower| {
            Some(FollowerInfo {
                user_id: follower["user_id"].as_str()?.to_string(),
                user_login: follower["user_login"].as_str()?.to_string(),
                user_name: follower["user_name"].as_str()?.to_string(),
                followed_at: DateTime::parse_from_rfc3339(follower["followed_at"].as_str()?).ok()?.with_timezone(&Utc),
            })
        })
        .collect();

    Ok((followers, total))
}