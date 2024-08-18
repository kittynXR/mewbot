use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use log::info;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::api::requests::get_channel_game;

pub async fn handle(
    event: &Value,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    api_client: &Arc<TwitchAPIClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let from_broadcaster_user_name = payload["from_broadcaster_user_name"].as_str().unwrap_or("Unknown");
        let viewers = payload["viewers"].as_u64().unwrap_or(0);
        let from_broadcaster_user_id = payload["from_broadcaster_user_id"].as_str().unwrap_or("0");

        info!("Raid received from: {} with {} viewers", from_broadcaster_user_name, viewers);

        // Get the game they were playing
        let game = get_channel_game(from_broadcaster_user_id, api_client).await?;

        let response = format!(
            "Welcome raiders! Thank you {} for the raid with {} {}! They were just playing {}. Hope you all had fun and enjoy your stay!",
            from_broadcaster_user_name,
            viewers,
            if viewers == 1 { "viewer" } else { "viewers" },
            game
        );

        irc_client.say(channel.to_string(), response).await?;
    }

    Ok(())
}