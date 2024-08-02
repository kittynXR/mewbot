use super::super::models::{Redemption, RedemptionResult};
use crate::osc::models::{OSCConfig, OSCMessageType, OSCValue};
use crate::osc::vrchat::VRChatOSC;


pub async fn handle_osc_message(
    redemption: &Redemption,
    osc_client: &VRChatOSC,
    osc_config: &OSCConfig
) -> RedemptionResult {
    let result = osc_client.send_osc_message_with_reset(osc_config).await;

    match result {
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