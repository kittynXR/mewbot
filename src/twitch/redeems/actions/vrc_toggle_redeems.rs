use std::any::Any;
use async_trait::async_trait;
use std::sync::Arc;
use log::{error, info, debug};
use tokio::time::Duration;
use crate::twitch::models::{Redemption, RedemptionResult, RedeemHandler};
use crate::osc::models::{OSCConfig, OSCMessageType, OSCValue};

pub struct VRCToggleRedeems {
    osc_manager: Arc<crate::osc::OSCManager>,
}

impl VRCToggleRedeems {
    pub fn new(osc_manager: Arc<crate::osc::OSCManager>) -> Self {
        Self { osc_manager }
    }

    async fn handle_evil_kittyn(&self, redemption: &Redemption) -> RedemptionResult {
        debug!("Processing evil kittyn toggle for {}", redemption.user_name);

        // Create OSC config for the evil kittyn parameter
        let osc_config = OSCConfig {
            uses_osc: true,
            osc_endpoint: "/avatar/parameters/evilkittyn".to_string(),
            osc_type: OSCMessageType::Boolean,
            osc_value: OSCValue::Boolean(true),
            default_value: OSCValue::Boolean(false),
            execution_duration: Some(72000), // 20 minutes * 60 frames per second
            send_chat_message: true,
        };

        // Send initial OSC message to enable evil kittyn
        match self.osc_manager.send_osc_message(
            &osc_config.osc_endpoint,
            &osc_config.osc_type,
            &osc_config.osc_value
        ).await {
            Ok(_) => {
                info!("Evil kittyn mode activated for {}", redemption.user_name);

                // Schedule the automatic disable after 20 minutes
                let osc_manager = self.osc_manager.clone();
                let endpoint = osc_config.osc_endpoint.clone();
                let msg_type = osc_config.osc_type;
                let default_value = osc_config.default_value;

                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(1200)).await; // 20 minutes
                    if let Err(e) = osc_manager.send_osc_message(&endpoint, &msg_type, &default_value).await {
                        error!("Failed to disable evil kittyn mode: {}", e);
                    } else {
                        info!("Evil kittyn mode automatically disabled after 20 minutes");
                    }
                });

                RedemptionResult {
                    success: true,
                    message: Some(format!("@{} has unleashed evil kittyn!", redemption.user_name)),
                }
            },
            Err(e) => {
                error!("Failed to activate evil kittyn mode: {}", e);
                RedemptionResult {
                    success: false,
                    message: Some("Failed to activate evil kittyn mode. Please try again later.".to_string()),
                }
            }
        }
    }

    async fn handle_fox_ears(&self, redemption: &Redemption) -> RedemptionResult {
        debug!("Processing fox ears toggle for {}", redemption.user_name);

        // Create OSC config for fox ears parameter
        let osc_config = OSCConfig {
            uses_osc: true,
            osc_endpoint: "/avatar/parameters/foxears".to_string(),
            osc_type: OSCMessageType::Boolean,
            osc_value: OSCValue::Boolean(true),
            default_value: OSCValue::Boolean(false),
            execution_duration: Some(3600), // 1 minute * 60 frames per second
            send_chat_message: true,
        };

        // Send initial OSC message to enable fox ears
        match self.osc_manager.send_osc_message(
            &osc_config.osc_endpoint,
            &osc_config.osc_type,
            &osc_config.osc_value
        ).await {
            Ok(_) => {
                info!("Fox ears activated for {}", redemption.user_name);

                // Schedule the automatic disable after 1 minute
                let osc_manager = self.osc_manager.clone();
                let endpoint = osc_config.osc_endpoint.clone();
                let msg_type = osc_config.osc_type;
                let default_value = osc_config.default_value;

                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(60)).await; // 1 minute
                    if let Err(e) = osc_manager.send_osc_message(&endpoint, &msg_type, &default_value).await {
                        error!("Failed to disable fox ears: {}", e);
                    } else {
                        info!("Fox ears automatically disabled after 1 minute");
                    }
                });

                RedemptionResult {
                    success: true,
                    message: Some(format!("@{} enabled fox ears for 1 minute! nyaa~", redemption.user_name)),
                }
            },
            Err(e) => {
                error!("Failed to activate fox ears: {}", e);
                RedemptionResult {
                    success: false,
                    message: Some("Failed to activate fox ears. Please try again later.".to_string()),
                }
            }
        }
    }
}

#[async_trait]
impl RedeemHandler for VRCToggleRedeems {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        debug!("Handling VRC toggle redeem: {}", redemption.reward_title);  // Add this debug line
        match redemption.reward_title.as_str() {
            "evil kittyn" => self.handle_evil_kittyn(redemption).await,
            "vrc fox ears" => self.handle_fox_ears(redemption).await,  // Make sure this matches exactly
            _ => {
                error!("Unknown VRC toggle redeem attempted: {}", redemption.reward_title);
                RedemptionResult {
                    success: false,
                    message: Some(format!("Unknown VRC toggle redeem: {}", redemption.reward_title)),
                }
            },
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}