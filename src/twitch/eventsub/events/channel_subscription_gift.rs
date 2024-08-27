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

        // Create OSC config for gift subs (same as resubs)
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
                    error!("Failed to send OSC message for gift sub: {}", e);
                }
            },
            None => {
                error!("VRChatOSC instance not available for gift sub event");
            }
        }

        let message = format!(
            "WOW! {} just gifted {} {} subscriptions! They've gifted a total of {} subs in the channel!",
            user_name, total, tier_name, cumulative_total
        );

        twitch_manager.send_message_as_bot(channel, &message).await?;

        info!("Processed gift sub event: {} gifted {} {} subs", user_name, total, tier_name);
    }

    Ok(())
}