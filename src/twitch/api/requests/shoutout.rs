use log::{info, error};
use std::sync::Arc;
use crate::twitch::api::client::TwitchAPIError;
use crate::twitch::TwitchManager;

#[derive(thiserror::Error, Debug)]
pub enum ShoutoutError {
    #[error("Failed to send shoutout: {0}")]
    APIError(#[from] TwitchAPIError),
    #[error("Invalid broadcaster ID: {0}")]
    InvalidBroadcasterId(String),
}

pub async fn send_shoutout(
    twitch_manager: &Arc<TwitchManager>,
    broadcaster_id: &str,
    moderator_id: &str,
    to_broadcaster_id: &str,
) -> Result<(), ShoutoutError> {
    info!("Attempting to send shoutout from {} to {}", broadcaster_id, to_broadcaster_id);

    if broadcaster_id.is_empty() || to_broadcaster_id.is_empty() {
        return Err(ShoutoutError::InvalidBroadcasterId("Broadcaster ID cannot be empty".into()));
    }

    let api_client = twitch_manager.get_api_client();

    match api_client.send_shoutout(broadcaster_id, to_broadcaster_id, moderator_id).await {
        Ok(_) => {
            info!("Shoutout successfully sent from {} to {}", broadcaster_id, to_broadcaster_id);
            Ok(())
        },
        Err(e) => {
            error!("Failed to send shoutout: {:?}", e);
            Err(ShoutoutError::APIError(e))
        }
    }
}