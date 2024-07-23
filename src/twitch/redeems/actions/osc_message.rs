use std::sync::Arc;
use async_trait::async_trait;
use crate::twitch::redeems::models::{Redemption, RedemptionResult};
use crate::osc::VRChatOSC;
use rosc::OscType;
use crate::ai::AIClient;
use crate::twitch::irc::client::TwitchIRCClientType;
use crate::twitch::redeems::{RedeemAction, RedeemManager};
use crate::twitch::TwitchAPIClient;

pub fn handle_osc_message(redemption: &Redemption, osc_client: &VRChatOSC) -> RedemptionResult {
    let user_input = redemption.user_input.clone().unwrap_or_default();

    // Parse the user input to get the address and value
    let parts: Vec<&str> = user_input.splitn(2, ' ').collect();
    if parts.len() != 2 {
        return RedemptionResult {
            success: false,
            message: Some("Invalid OSC message format. Use: <address> <value>".to_string()),
            queue_number: redemption.queue_number,
        };
    }

    let address = parts[0];
    let value = parts[1];

    // Convert the value to OscType (you might want to add more type conversions)
    let osc_value = if let Ok(f) = value.parse::<f32>() {
        OscType::Float(f)
    } else {
        OscType::String(value.to_string())
    };

    match osc_client.send_message(address, osc_value) {
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

pub struct OSCMessageAction;
#[async_trait]
impl RedeemAction for OSCMessageAction {
    async fn execute(&self, redemption: &Redemption, api_client: &TwitchAPIClient, irc_client: &Arc<TwitchIRCClientType>, channel: &str, ai_client: Option<&AIClient>, osc_client: Option<&VRChatOSC>, redeem_manager: &RedeemManager) -> RedemptionResult {
        // if let Some(osc_client) = osc_client {
        //     // Create a temporary RedeemManager instance to call handle_osc_message
        //     let temp_manager = RedeemManager::new(
        //         None,
        //         Some(Arc::new(osc_client.clone())),
        //         Arc::new(api_client.clone())
        //     );
        //     temp_manager.handle_osc_message(redemption).await
        // } else {
            RedemptionResult {
                success: false,
                message: Some("OSC client not initialized".to_string()),
                queue_number: redemption.queue_number,
            // }
        }
    }
}