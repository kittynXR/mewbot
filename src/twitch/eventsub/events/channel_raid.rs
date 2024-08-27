use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use log::{debug, info};
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::api::requests::get_channel_game;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::TwitchManager;

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let from_broadcaster_user_name = payload["from_broadcaster_user_name"].as_str().unwrap_or("Unknown");
        let viewers = payload["viewers"].as_u64().unwrap_or(0);
        let from_broadcaster_user_id = payload["from_broadcaster_user_id"].as_str().unwrap_or("0");
        let api_client = twitch_manager.get_api_client();
        debug!("Raid received from: {} with {} viewers", from_broadcaster_user_name, viewers);

        // Get the game they were playing
        let game = get_channel_game(from_broadcaster_user_id, &api_client).await?;

        let response = format!(
            "Welcome raiders! Thank you {} for the raid with {} {}! They were just playing {}. Hope you all had fun and enjoy your stay!",
            from_broadcaster_user_name,
            viewers,
            if viewers == 1 { "viewer" } else { "viewers" },
            game
        );

        twitch_manager.send_message_as_bot(channel, response.as_str()).await?;
    }

    Ok(())
}