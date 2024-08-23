use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use log::{info, error};
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::TwitchManager;

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        if let Some(user_name) = payload["user_name"].as_str() {
            info!("New follower: {}", user_name);

            let api_client = twitch_manager.get_api_client();
            // Get the broadcaster ID
            let broadcaster_id = api_client.get_broadcaster_id().await?;

            // Get the current follower count
            match api_client.get_follower_count(&broadcaster_id).await {
                Ok(follower_count) => {
                    let response = format!(
                        "Thank you for following, {}! Welcome to the community! soul#{} mao mao",
                        user_name,
                        follower_count
                    );

                    twitch_manager.send_message_as_bot(channel, response.as_str()).await?;
                },
                Err(e) => {
                    error!("Failed to get follower count: {:?}", e);
                    // Fall back to the original message if we can't get the follower count
                    let response = format!("Thank you for following, {}! Welcome to the community! mao mao", user_name);
                    twitch_manager.send_message_as_bot(channel, response.as_str()).await?;
                }
            }
        }
    }

    Ok(())
}