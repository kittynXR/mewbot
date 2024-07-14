use chrono::{DateTime, Utc, Duration};
use reqwest::Client;
use serde_json::Value;

pub async fn get_stream_uptime(channel: &str) -> Result<Option<Duration>, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();
    let response = client.get(format!("https://api.twitch.tv/helix/streams?user_login={}", channel))
        .header("Client-ID", "your_client_id_here")
        .header("Authorization", "Bearer your_access_token_here")
        .send()
        .await?
        .json::<Value>()
        .await?;

    if let Some(stream_data) = response["data"].as_array().and_then(|arr| arr.first()) {
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