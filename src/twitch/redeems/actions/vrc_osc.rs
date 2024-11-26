use std::any::Any;
use async_trait::async_trait;
use std::sync::Arc;
use log::{debug, error, warn};
use crate::twitch::models::{Redemption, RedemptionResult, RedeemHandler};
use crate::twitch::TwitchManager;

#[derive(Clone)]
pub struct VRCOscRedeems {
    twitch_manager: Arc<TwitchManager>,
}

impl VRCOscRedeems {
    pub fn new(twitch_manager: Arc<TwitchManager>) -> Self {
        Self { twitch_manager }
    }

    async fn handle_osc_redeem(&self, redemption: &Redemption) -> RedemptionResult {
        let osc_manager = self.twitch_manager.get_osc_manager();

        if !osc_manager.is_connected().await {
            warn!("OSC is not connected. Attempting to reconnect...");
            if let Err(e) = osc_manager.reconnect().await {
                error!("Failed to reconnect OSC: {}", e);
                return RedemptionResult {
                    success: false,
                    message: Some("OSC is not connected and reconnection failed".to_string()),
                };
            }
        }

        let osc_configs = self.twitch_manager.get_osc_configs();
        let configs = osc_configs.read().await;
        debug!("Available OSC configs: {:?}", configs.configs.keys().collect::<Vec<_>>());

        if let Some(config) = configs.get_config(&redemption.reward_title) {
            debug!("Found OSC config for {}: {:?}", redemption.reward_title, config);
            if config.uses_osc {
                match osc_manager.send_osc_message(&config.osc_endpoint, &config.osc_type, &config.osc_value).await {
                    Ok(_) => {
                        if let Some(frames) = config.execution_duration {
                            // Clone necessary values for the background task
                            let osc_manager = osc_manager.clone();
                            let osc_endpoint = config.osc_endpoint.clone();
                            let osc_type = config.osc_type.clone();
                            let default_value = config.default_value.clone();
                            let reward_title = redemption.reward_title.clone();

                            // Spawn a background task for the timer
                            tokio::spawn(async move {
                                let duration = std::time::Duration::from_secs_f32(frames as f32 / 60.0);
                                tokio::time::sleep(duration).await;
                                if let Err(e) = osc_manager.send_osc_message(&osc_endpoint, &osc_type, &default_value).await {
                                    error!("Failed to reset OSC value for {}: {}", reward_title, e);
                                }
                            });
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
            "toss pillo" | "cream pie" | "water balloon" | "cat trap" | "snowball" | "share loli slam" | "gib cookie" | "leash" => {
                self.handle_osc_redeem(redemption).await
            },
            _ => RedemptionResult {
                success: false,
                message: Some(format!("Unknown VRChat OSC redeem: {}", redemption.reward_title)),
            },
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}