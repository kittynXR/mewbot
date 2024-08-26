use async_trait::async_trait;
use std::sync::Arc;
use log::{debug, error, warn};
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
        debug!("Available OSC configs: {:?}", configs.configs.keys().collect::<Vec<_>>());

        // Change this line
        if let Some(config) = configs.get_config(&redemption.reward_title) {
            debug!("Found OSC config for {}: {:?}", redemption.reward_title, config);
            if config.uses_osc {
                match self.vrchat_osc.send_osc_message_with_reset(config).await {
                    Ok(_) => RedemptionResult {
                        success: true,
                        message: None,
                    },
                    Err(e) => {
                        error!("Failed to send OSC message for {}: {}", redemption.reward_title, e);
                        RedemptionResult {
                            success: false,
                            message: Some(format!("Failed to send OSC message: {}", e)),
                        }
                    }
                }
            } else {
                warn!("{} action is not configured to use OSC", redemption.reward_title);
                RedemptionResult {
                    success: false,
                    message: Some(format!("{} action is not configured to use OSC", redemption.reward_title)),
                }
            }
        } else {
            error!("{} OSC configuration not found. Available configs: {:?}", redemption.reward_title, configs.configs.keys().collect::<Vec<_>>());
            RedemptionResult {
                success: false,
                message: Some(format!("{} OSC configuration not found", redemption.reward_title)),
            }
        }
    }
}