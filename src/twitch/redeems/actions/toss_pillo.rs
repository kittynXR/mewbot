use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::osc::VRChatOSC;
use crate::osc::osc_config::OSCConfigurations;
use crate::twitch::redeems::models::{Redemption, RedemptionResult, RedeemHandler};

pub struct TossPillowAction {
    vrchat_osc: Arc<VRChatOSC>,
    osc_configs: Arc<RwLock<OSCConfigurations>>,
}

impl TossPillowAction {
    pub fn new(vrchat_osc: Arc<VRChatOSC>, osc_configs: Arc<RwLock<OSCConfigurations>>) -> Self {
        Self { vrchat_osc, osc_configs }
    }
}

#[async_trait]
impl RedeemHandler for TossPillowAction {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        let configs = self.osc_configs.read().await;
        if let Some(config) = configs.get_config("TossPillow") {
            if config.uses_osc {
                match self.vrchat_osc.send_osc_message_with_reset(config).await {
                    Ok(_) => RedemptionResult {
                        success: true,
                        message: Some("Pillow tossed!".to_string()),
                    },
                    Err(e) => RedemptionResult {
                        success: false,
                        message: Some(format!("Failed to send OSC message: {}", e)),
                    },
                }
            } else {
                RedemptionResult {
                    success: false,
                    message: Some("Toss Pillow action is not configured to use OSC".to_string()),
                }
            }
        } else {
            RedemptionResult {
                success: false,
                message: Some("Toss Pillow OSC configuration not found".to_string()),
            }
        }
    }
}