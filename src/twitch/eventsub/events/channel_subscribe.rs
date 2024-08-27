use serde_json::Value;
use std::sync::Arc;
use log::error;
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

        // Create OSC config for new subscribers
        let osc_config = OSCConfig {
            uses_osc: true,
            osc_endpoint: "/avatar/parameters/twitch".to_string(),
            osc_type: OSCMessageType::Integer,
            osc_value: OSCValue::Integer(13),
            default_value: OSCValue::Integer(0),
            execution_duration: Some(std::time::Duration::from_secs(1)),
            send_chat_message: false,
        };

        // Send OSC message using VRChatOSC through the get_vrchat_osc method
        match twitch_manager.get_vrchat_osc() {
            Some(vrchat_osc) => {
                if let Err(e) = vrchat_osc.send_osc_message_with_reset(&osc_config).await {
                    error!("Failed to send OSC message for new subscriber: {}", e);
                }
            },
            None => {
                error!("VRChatOSC instance not available");
            }
        }

        let message = if is_gift {
            format!("{} received a gifted {} subscription! Thank you to the generous gifter!", user_name, tier_name)
        } else {
            format!("Thank you {} for subscribing with a {} subscription!", user_name, tier_name)
        };

        twitch_manager.send_message_as_bot(channel, &message).await?;
    }

    Ok(())
}