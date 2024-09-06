use serde_json::Value;
use std::sync::Arc;
use log::{error, info, debug};
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

        debug!("Received resub event: {} ({} months)", user_name, cumulative_months);

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

        // Send OSC message using the OSCManager
        let osc_manager = twitch_manager.get_osc_manager();
        match osc_manager.send_osc_message(&osc_config.osc_endpoint, &osc_config.osc_type, &osc_config.osc_value).await {
            Ok(_) => {
                debug!("Successfully sent OSC message for resub event");
                // Reset the OSC value after the execution duration
                if let Some(duration) = osc_config.execution_duration {
                    tokio::time::sleep(duration).await;
                    if let Err(e) = osc_manager.send_osc_message(&osc_config.osc_endpoint, &osc_config.osc_type, &osc_config.default_value).await {
                        error!("Failed to reset OSC value for resub event: {}", e);
                    }
                }
            },
            Err(e) => error!("Failed to send OSC message for resub event: {}", e),
        }

        let response = format!("Thank you {} for {} months of support! They said: {}", user_name, cumulative_months, message);
        if let Err(e) = twitch_manager.send_message_as_bot(channel, &response).await {
            error!("Failed to send thank you message to chat: {}", e);
        } else {
            debug!("Successfully sent thank you message to chat");
        }

        info!("Processed resub event for {} ({} months)", user_name, cumulative_months);
    } else {
        error!("Invalid event payload structure");
    }

    Ok(())
}