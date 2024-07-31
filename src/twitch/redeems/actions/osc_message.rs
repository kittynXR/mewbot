use super::super::models::{Redemption, OSCConfig, RedemptionResult};
use crate::osc::vrchat::VRChatOSC;

pub async fn handle_osc_message(
    redemption: &Redemption,
    osc_client: &VRChatOSC,
    osc_config: &OSCConfig
) -> RedemptionResult {
    let message = match &redemption.user_input {
        Some(input) => input.clone(),
        None => redemption.reward_title.clone(),
    };

    match osc_client.send_chatbox_message(&message, true, true) {
        Ok(_) => RedemptionResult {
            success: true,
            message: Some(format!("OSC message sent successfully for redemption: {}", redemption.reward_title)),
            queue_number: redemption.queue_number,
        },
        Err(e) => RedemptionResult {
            success: false,
            message: Some(format!("Failed to send OSC message: {}", e)),
            queue_number: redemption.queue_number,
        },
    }
}