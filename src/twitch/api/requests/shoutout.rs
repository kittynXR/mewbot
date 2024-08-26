use std::sync::Arc;
use crate::twitch::TwitchManager;

pub async fn send_shoutout(
    twitch_manager: &Arc<TwitchManager>,
    broadcaster_id: &str,
    moderator_id: &str,
    to_broadcaster_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let api_client = twitch_manager.get_api_client();

    api_client.send_shoutout(broadcaster_id, to_broadcaster_id, moderator_id).await
}