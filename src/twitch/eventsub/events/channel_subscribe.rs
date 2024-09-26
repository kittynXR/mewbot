use serde_json::Value;
use std::sync::Arc;
use log::{error, debug, info};
use crate::twitch::TwitchManager;
use crate::osc::models::OSCConfig;
use crate::osc::models::{OSCMessageType, OSCValue};

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let user_name = payload["user_name"].as_str().unwrap_or("Unknown");
        let tier = payload["tier"].as_str().unwrap_or("1000");
        let is_gift = payload["is_gift"].as_bool().unwrap_or(false);

        let tier_name = match tier {
            "1000" => "Tier 1",
            "2000" => "Tier 2",
            "3000" => "Tier 3",
            _ => "Unknown Tier",
        };

        debug!("Received subscribe event: {} - {} (Gift: {})", user_name, tier_name, is_gift);

        // Create OSC config for new subscribers
        let osc_config = OSCConfig {
            uses_osc: true,
            osc_endpoint: "/avatar/parameters/twitch".to_string(),
            osc_type: OSCMessageType::Integer,
            osc_value: OSCValue::Integer(13),
            default_value: OSCValue::Integer(0),
            execution_duration: Some(60),
            send_chat_message: false,
        };

        // Send OSC message using the OSCManager
        let osc_manager = twitch_manager.get_osc_manager();
        match osc_manager.send_osc_message(&osc_config.osc_endpoint, &osc_config.osc_type, &osc_config.osc_value).await {
            Ok(_) => {
                debug!("Successfully sent OSC message for new subscriber");
                // Reset the OSC value after the execution duration
                if let Some(frames) = osc_config.execution_duration {
                    let duration = std::time::Duration::from_secs_f32(frames as f32 / 60.0);
                    tokio::time::sleep(duration).await;
                    if let Err(e) = osc_manager.send_osc_message(&osc_config.osc_endpoint, &osc_config.osc_type, &osc_config.default_value).await {
                        error!("Failed to reset OSC value for new subscriber: {}", e);
                    }
                }
            },
            Err(e) => error!("Failed to send OSC message for new subscriber: {}", e),
        }

        let message = if is_gift {
            format!("{} received a gifted {} subscription! Thank you to the generous gifter!", user_name, tier_name)
        } else {
            format!("Thank you {} for subscribing with a {} subscription!", user_name, tier_name)
        };

        if let Err(e) = twitch_manager.send_message_as_bot(channel, &message).await {
            error!("Failed to send thank you message to chat: {}", e);
        } else {
            debug!("Successfully sent thank you message to chat");
        }

        info!("Processed subscribe event for {} - {} (Gift: {})", user_name, tier_name, is_gift);
    } else {
        error!("Invalid event payload structure");
    }

    Ok(())
}