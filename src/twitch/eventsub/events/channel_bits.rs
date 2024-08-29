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
        let user_name = payload["user_name"].as_str().unwrap_or("Anonymous");
        let bits_used = payload["bits_used"].as_i64().unwrap_or(0) as i32;

        debug!("Received bits event: {} bits from {}", bits_used, user_name);

        // Determine which OSC action to use based on the number of bits
        let osc_value = match bits_used {
            1..=99 => 14,
            100..=999 => 15,
            1000..=4999 => 16,
            5000..=9999 => 17,
            10000..=14999 => 18,
            _ => 19,
        };

        debug!("Determined OSC value: {}", osc_value);

        // Create OSC config
        let osc_config = OSCConfig {
            uses_osc: true,
            osc_endpoint: "/avatar/parameters/twitch".to_string(),
            osc_type: OSCMessageType::Integer,
            osc_value: OSCValue::Integer(osc_value),
            default_value: OSCValue::Integer(0),
            execution_duration: Some(std::time::Duration::from_secs(1)),
            send_chat_message: false,
        };

        // Send OSC message
        match twitch_manager.get_vrchat_osc() {
            Some(vrchat_osc) => {
                match vrchat_osc.send_osc_message_with_reset(&osc_config).await {
                    Ok(_) => debug!("Successfully sent OSC message for bits event"),
                    Err(e) => error!("Failed to send OSC message for bits event: {}", e),
                }
            },
            None => {
                error!("VRChatOSC instance not available for bits event");
            }
        }

        // Send chat message
        let message = format!("Thank you {} for the {} bits!", user_name, bits_used);
        match twitch_manager.send_message_as_bot(channel, &message).await {
            Ok(_) => debug!("Successfully sent thank you message to chat"),
            Err(e) => error!("Failed to send thank you message to chat: {}", e),
        }

        info!("Processed bit cheer event for {} bits from {}", bits_used, user_name);
    } else {
        error!("Invalid event payload structure");
    }

    Ok(())
}