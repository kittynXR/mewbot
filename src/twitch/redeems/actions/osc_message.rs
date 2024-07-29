// File: src/twitch/redeems/actions/osc_message.rs

use crate::twitch::redeems::models::{Redemption, RedemptionResult, OSCConfig};
use crate::osc::VRChatOSC;
use rosc::OscType;

pub fn handle_osc_message(redemption: &Redemption, osc_client: &VRChatOSC, osc_config: &OSCConfig) -> RedemptionResult {
    let value = redemption.user_input.clone().unwrap_or_default();

    // Convert the value to OscType (you might want to add more type conversions)
    let osc_value = if let Ok(f) = value.parse::<f32>() {
        OscType::Float(f)
    } else {
        OscType::String(value)
    };

    match osc_client.send_message(&osc_config.osc_endpoint, osc_value) {
        Ok(_) => RedemptionResult {
            success: true,
            message: Some("OSC message sent successfully".to_string()),
            queue_number: redemption.queue_number,
        },
        Err(e) => RedemptionResult {
            success: false,
            message: Some(format!("Failed to send OSC message: {}", e)),
            queue_number: redemption.queue_number,
        },
    }
}