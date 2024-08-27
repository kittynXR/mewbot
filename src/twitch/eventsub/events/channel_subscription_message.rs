use serde_json::Value;
use std::sync::Arc;
use log::{error, info};
use crate::twitch::TwitchManager;
use crate::osc::models::{OSCConfig, OSCMessageType, OSCValue};

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let user_name = payload["user_name"].as_str().unwrap_or("Unknown");
        let message = payload["message"].as_str().unwrap_or("");
        let cumulative_months = payload["cumulative_months"].as_u64().unwrap_or(0);

        // Create OSC config for resubscribers
        let osc_config = OSCConfig {
            uses_osc: true,
            osc_endpoint: "/avatar/parameters/twitch".to_string(),
            osc_type: OSCMessageType::Integer,
            osc_value: OSCValue::Integer(20),
            default_value: OSCValue::Integer(0),
            execution_duration: Some(std::time::Duration::from_secs(1)),
            send_chat_message: false,
        };

        // Send OSC message
        match twitch_manager.get_vrchat_osc() {
            Some(vrchat_osc) => {
                if let Err(e) = vrchat_osc.send_osc_message_with_reset(&osc_config).await {
                    error!("Failed to send OSC message for resubscriber: {}", e);
                }
            },
            None => {
                error!("VRChatOSC instance not available for resub event");
            }
        }

        let response = format!("Thank you {} for {} months of support! They said: {}", user_name, cumulative_months, message);
        twitch_manager.send_message_as_bot(channel, &response).await?;

        info!("Processed resub event for {} ({} months)", user_name, cumulative_months);
    }

    Ok(())
}