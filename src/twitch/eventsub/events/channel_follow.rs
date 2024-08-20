use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use log::{info, error};
use crate::twitch::api::TwitchAPIClient;

pub async fn handle(
    event: &Value,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    api_client: &Arc<TwitchAPIClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        if let Some(user_name) = payload["user_name"].as_str() {
            info!("New follower: {}", user_name);

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

                    irc_client.say(channel.to_string(), response).await?;
                },
                Err(e) => {
                    error!("Failed to get follower count: {:?}", e);
                    // Fall back to the original message if we can't get the follower count
                    let response = format!("Thank you for following, {}! Welcome to the community! mao mao", user_name);
                    irc_client.say(channel.to_string(), response).await?;
                }
            }
        }
    }

    Ok(())
}