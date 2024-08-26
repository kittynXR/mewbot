use serde_json::Value;
use std::sync::Arc;
use crate::twitch::TwitchManager;
use crate::ai::AIClient;

pub async fn handle_ad_break_begin(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let duration_seconds = payload["duration_seconds"].as_str().unwrap_or("0").parse::<u32>().unwrap_or(0);
        let is_automatic = payload["is_automatic"].as_str().unwrap_or("false") == "true";

        // Generate a friendly AI message
        let ai_client = AIClient::new(None, None); // You might want to pass actual API keys here
        let prompt = format!(
            "Generate a short, friendly message (max 100 characters) for a Twitch streamer to say when an ad break of {} seconds begins. {}",
            duration_seconds,
            if is_automatic { "This ad break was automatically scheduled." } else { "This ad break was manually triggered by the streamer." }
        );
        let ai_message = ai_client.generate_response_without_history(&prompt).await
            .unwrap_or_else(|_| "We'll be right back after this short break!".to_string());

        let message = format!(
            "An ad break has begun for {} seconds. {}. Subscribe to watch ad-free!",
            duration_seconds, ai_message
        );

        twitch_manager.send_message_as_bot(channel, &message).await?;
    }

    Ok(())
}