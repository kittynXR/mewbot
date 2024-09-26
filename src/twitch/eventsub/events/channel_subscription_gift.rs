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
        let total = payload["total"].as_u64().unwrap_or(0);
        let tier = payload["tier"].as_str().unwrap_or("1000");
        let cumulative_total = payload["cumulative_total"].as_u64().unwrap_or(0);

        let tier_name = match tier {
            "1000" => "Tier 1",
            "2000" => "Tier 2",
            "3000" => "Tier 3",
            _ => "Unknown Tier",
        };

        debug!("Received gift sub event: {} gifted {} {} subs (Total: {})", user_name, total, tier_name, cumulative_total);

        // Create OSC config for gift subs
        let osc_config = OSCConfig {
            uses_osc: true,
            osc_endpoint: "/avatar/parameters/twitch".to_string(),
            osc_type: OSCMessageType::Integer,
            osc_value: OSCValue::Integer(20),
            default_value: OSCValue::Integer(0),
            execution_duration: Some(60),
            send_chat_message: false,
        };

        // Send OSC message using the OSCManager
        let osc_manager = twitch_manager.get_osc_manager();
        match osc_manager.send_osc_message(&osc_config.osc_endpoint, &osc_config.osc_type, &osc_config.osc_value).await {
            Ok(_) => {
                debug!("Successfully sent OSC message for gift sub event");
                // Reset the OSC value after the execution duration
                if let Some(frames) = osc_config.execution_duration {
                    let duration = std::time::Duration::from_secs_f32(frames as f32 / 60.0);
                    tokio::time::sleep(duration).await;
                    if let Err(e) = osc_manager.send_osc_message(&osc_config.osc_endpoint, &osc_config.osc_type, &osc_config.default_value).await {
                        error!("Failed to reset OSC value for gift sub event: {}", e);
                    }
                }
            },
            Err(e) => error!("Failed to send OSC message for gift sub event: {}", e),
        }

        let message = format!(
            "WOW! {} just gifted {} {} subscriptions! They've gifted a total of {} subs in the channel!",
            user_name, total, tier_name, cumulative_total
        );

        if let Err(e) = twitch_manager.send_message_as_bot(channel, &message).await {
            error!("Failed to send thank you message to chat: {}", e);
        } else {
            debug!("Successfully sent thank you message to chat");
        }

        info!("Processed gift sub event: {} gifted {} {} subs", user_name, total, tier_name);
    } else {
        error!("Invalid event payload structure");
    }

    Ok(())
}