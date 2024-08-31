use async_trait::async_trait;
use std::sync::Arc;
use log::{debug, error, warn};
use tokio::sync::RwLock;
use crate::osc::VRChatOSC;
use crate::osc::osc_config::OSCConfigurations;
use crate::twitch::redeems::models::{Redemption, RedemptionResult, RedeemHandler};

use crate::osc::OSCManager;

pub struct VRCOscRedeems {
    osc_manager: Arc<OSCManager>,
    osc_configs: Arc<RwLock<OSCConfigurations>>,
}

impl VRCOscRedeems {
    pub fn new(osc_manager: Arc<OSCManager>, osc_configs: Arc<RwLock<OSCConfigurations>>) -> Self {
        Self { osc_manager, osc_configs }
    }

    async fn handle_osc_redeem(&self, redemption: &Redemption) -> RedemptionResult {
        if !self.osc_manager.is_connected().await {
            warn!("OSC is not connected. Attempting to reconnect...");
            if let Err(e) = self.osc_manager.reconnect().await {
                error!("Failed to reconnect OSC: {}", e);
                return RedemptionResult {
                    success: false,
                    message: Some("OSC is not connected and reconnection failed".to_string()),
                };
            }
        }

        let configs = self.osc_configs.read().await;
        debug!("Available OSC configs: {:?}", configs.configs.keys().collect::<Vec<_>>());

        if let Some(config) = configs.get_config(&redemption.reward_title) {
            debug!("Found OSC config for {}: {:?}", redemption.reward_title, config);
            if config.uses_osc {
                match self.osc_manager.send_osc_message(&config.osc_endpoint, &config.osc_type, &config.osc_value).await {
                    Ok(_) => {
                        if let Some(duration) = config.execution_duration {
                            tokio::time::sleep(duration).await;
                            if let Err(e) = self.osc_manager.send_osc_message(&config.osc_endpoint, &config.osc_type, &config.default_value).await {
                                error!("Failed to reset OSC value for {}: {}", redemption.reward_title, e);
                            }
                        }
                        RedemptionResult {
                            success: true,
                            message: None,
                        }
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

#[async_trait]
impl RedeemHandler for VRCOscRedeems {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        match redemption.reward_title.as_str() {
            "toss pillo" | "cream pie" | "water balloon" | "cat trap" | "snowball" => {
                self.handle_osc_redeem(redemption).await
            },
            _ => RedemptionResult {
                success: false,
                message: Some(format!("Unknown VRChat OSC redeem: {}", redemption.reward_title)),
            },
        }
    }
}