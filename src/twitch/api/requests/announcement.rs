use crate::twitch::api::TwitchAPIClient;

pub async fn send_announcement(
    api_client: &TwitchAPIClient,
    broadcaster_id: &str,
    moderator_id: &str,
    message: &str,
    color: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;

    let mut json_body = serde_json::json!({
        "broadcaster_id": broadcaster_id,
        "moderator_id": moderator_id,
        "message": message,
    });

    if let Some(c) = color {
        json_body["color"] = serde_json::json!(c);
    }

    let response = api_client.client
        .post("https://api.twitch.tv/helix/chat/announcements")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .json(&json_body)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to send announcement. Status: {}, Error: {}", status, error_text).into());
    }

    Ok(())
}