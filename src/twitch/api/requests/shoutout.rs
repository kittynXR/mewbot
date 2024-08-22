use std::sync::Arc;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::TwitchManager;

pub async fn send_shoutout(
    twitch_manager: &Arc<TwitchManager>,
    broadcaster_id: &str,
    moderator_id: &str,
    to_broadcaster_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let api_client = twitch_manager.get_api_client();

    let token = api_client.get_token().await?;
    let client_id = api_client.get_client_id().await?;
    let api_client = twitch_manager.get_api_client();
    let response = api_client.client
        .post("https://api.twitch.tv/helix/chat/shoutouts")
        .header("Client-ID", client_id)
        .header("Authorization", format!("Bearer {}", token))
        .json(&serde_json::json!({
            "from_broadcaster_id": broadcaster_id,
            "to_broadcaster_id": to_broadcaster_id,
            "moderator_id": moderator_id,
        }))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let error_text = response.text().await?;
        return Err(format!("Failed to send shoutout. Status: {}, Error: {}", status, error_text).into());
    }

    Ok(())
}