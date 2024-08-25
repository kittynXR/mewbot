// src/twitch/api/requests/channel_points.rs

use crate::twitch::api::TwitchAPIClient;
use serde_json::json;

pub async fn update_redemption_status(
    api_client: &TwitchAPIClient,
    broadcaster_id: &str,
    reward_id: &str,
    redemption_id: &str,
    status: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let response = api_client.client
        .patch("https://api.twitch.tv/helix/channel_points/custom_rewards/redemptions")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .query(&[
            ("broadcaster_id", broadcaster_id),
            ("reward_id", reward_id),
            ("id", redemption_id),
        ])
        .json(&json!({
            "status": status
        }))
        .send()
        .await?;

    let status_code = response.status();
    if !status_code.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to update redemption status. Status: {}, Error: {}", status_code, error_text).into());
    }

    Ok(())
}

pub async fn refund_channel_points(
    api_client: &TwitchAPIClient,
    broadcaster_id: &str,
    reward_id: &str,
    redemption_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    update_redemption_status(api_client, broadcaster_id, reward_id, redemption_id, "CANCELED").await
}

pub async fn get_custom_reward(
    api_client: &TwitchAPIClient,
    broadcaster_id: &str,
    reward_id: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let response = api_client.client
        .get("https://api.twitch.tv/helix/channel_points/custom_rewards")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .query(&[
            ("broadcaster_id", broadcaster_id),
            ("id", reward_id),
        ])
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to get custom reward. Status: {}, Error: {}", status, error_text).into());
    }

    let body: serde_json::Value = response.json().await?;
    Ok(body)
}

pub async fn delete_custom_reward(
    api_client: &TwitchAPIClient,
    broadcaster_id: &str,
    reward_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let response = api_client.client
        .delete("https://api.twitch.tv/helix/channel_points/custom_rewards")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .query(&[
            ("broadcaster_id", broadcaster_id),
            ("id", reward_id),
        ])
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to delete custom reward. Status: {}, Error: {}", status, error_text).into());
    }

    Ok(())
}