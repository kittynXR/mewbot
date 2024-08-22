use std::sync::Arc;
use chrono::{DateTime, Utc, Duration};
use log::{debug, error};
use crate::twitch::TwitchAPIClient;
use serde_json::Value;

pub async fn get_stream_uptime(channel: &str, api_client: Arc<TwitchAPIClient>) -> Result<Option<Duration>, Box<dyn std::error::Error + Send + Sync>> {
    debug!("Attempting to get stream uptime for channel: {}", channel);

    let access_token = match api_client.get_token().await {
        Ok(token) => token,
        Err(e) => {
            error!("Failed to get access token: {:?}", e);
            return Err(e.into());
        }
    };

    debug!("Successfully obtained access token");

    let client_id = match api_client.get_client_id().await {
        Ok(id) => id,
        Err(e) => {
            error!("Failed to get client ID: {:?}", e);
            return Err(e.into());
        }
    };

    debug!("Successfully obtained client ID");

    let client = reqwest::Client::new();
    let response = client.get(format!("https://api.twitch.tv/helix/streams?user_login={}", channel))
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?;

    debug!("API Response status: {}", response.status());

    let response_body = response.text().await?;
    debug!("API Response body: {}", response_body);

    let json: Value = serde_json::from_str(&response_body)?;

    if let Some(stream_data) = json["data"].as_array().and_then(|arr| arr.first()) {
        if let Some(started_at) = stream_data["started_at"].as_str() {
            let start_time = DateTime::parse_from_rfc3339(started_at)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
                .with_timezone(&Utc);
            let now = Utc::now();
            return Ok(Some(now.signed_duration_since(start_time)));
        }
    }

    Ok(None)
}