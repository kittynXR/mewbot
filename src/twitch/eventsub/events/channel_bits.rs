use serde_json::Value;
use std::sync::Arc;
use log::{error, info, debug, warn};
use crate::twitch::TwitchManager;
use crate::osc::models::{OSCConfig, OSCMessageType, OSCValue};

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let user_name = payload["user_name"].as_str().unwrap_or("Anonymous");

        // More robust extraction of bits_used
        let bits_used = match payload["bits"].as_u64() {
            Some(bits) => bits,
            None => {
                warn!("Failed to extract bits amount from payload: {:?}", payload);
                return Ok(());  // Exit early if we can't get the bits amount
            }
        };

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
            osc_value: OSCValue::Integer(osc_value as i32),
            default_value: OSCValue::Integer(0),
            execution_duration: Some(std::time::Duration::from_secs(1)),
            send_chat_message: false,
        };

        // Send OSC message using the OSCManager
        let osc_manager = twitch_manager.get_osc_manager();
        match osc_manager.send_osc_message(&osc_config.osc_endpoint, &osc_config.osc_type, &osc_config.osc_value).await {
            Ok(_) => {
                debug!("Successfully sent OSC message for bits event");
                // Reset the OSC value after the execution duration
                if let Some(duration) = osc_config.execution_duration {
                    tokio::time::sleep(duration).await;
                    if let Err(e) = osc_manager.send_osc_message(&osc_config.osc_endpoint, &osc_config.osc_type, &osc_config.default_value).await {
                        error!("Failed to reset OSC value for bits event: {}", e);
                    }
                }
            },
            Err(e) => error!("Failed to send OSC message for bits event: {}", e),
        }

        // Send chat message
        let message = format!("Thank you {} for the {} bits!", user_name, bits_used);
        if let Err(e) = twitch_manager.send_message_as_bot(channel, &message).await {
            error!("Failed to send thank you message to chat: {}", e);
        } else {
            debug!("Successfully sent thank you message to chat: {}", message);
        }

        info!("Processed bit cheer event for {} bits from {}", bits_used, user_name);
    } else {
        error!("Invalid event payload structure");
    }

    Ok(())
}