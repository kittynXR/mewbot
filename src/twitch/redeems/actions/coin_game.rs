// In coin_game.rs

use std::any::Any;
use std::sync::Arc;
use async_trait::async_trait;
use log::{debug, error, info};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tokio::sync::{Mutex, RwLock};
use crate::twitch::models::{CoinGameState, RedeemHandler, Redemption, RedemptionResult};
use crate::twitch::redeems::RedeemManager;

pub struct CoinGameAction {
    state: Arc<RwLock<CoinGameState>>,
    redeem_manager: Arc<RedeemManager>,
    rng: Arc<Mutex<StdRng>>,
}

impl CoinGameAction {
    pub fn new(state: Arc<RwLock<CoinGameState>>, redeem_manager: Arc<RedeemManager>) -> Self {
        Self {
            state,
            redeem_manager,
            rng: Arc::new(Mutex::new(StdRng::from_entropy())),
        }
    }

    async fn generate_silly_message(&self, previous_redeemer: &Option<(Redemption, u32)>, current_price: u32) -> String {
        let prompt = match previous_redeemer {
            Some((redeemer, cost)) => format!(
                "Generate a silly message for a Twitch coin game. The previous redeemer was {} who redeemed for {} points. The new price is {} points. Keep it short and fun!",
                redeemer.user_name, cost, current_price
            ),
            None => format!(
                "Generate a silly message to start a Twitch coin game. The starting price is {} points. Keep it short and fun!",
                current_price
            ),
        };

        self.redeem_manager.get_ai_client().generate_response_without_history(&prompt).await
            .unwrap_or_else(|_| format!("Coin game! New price: {} points", current_price))
    }

    async fn update_reward(&self, reward_id: &str, new_price: u32, new_prompt: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.redeem_manager.get_api_client().update_custom_reward(
            reward_id,
            "Coin Game",
            new_price,
            true,
            0,
            new_prompt,
            false,
        ).await?;
        Ok(())
    }

    async fn refund_previous_redeemer(&self, redemption: &Redemption) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Err(e) = self.redeem_manager.get_api_client().refund_channel_points(
            &redemption.reward_id,
            &redemption.id,
        ).await {
            error!("Failed to refund previous redeemer: {:?}", e);
            return Err(e);
        }
        Ok(())
    }

    pub async fn handle_offline(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.state.write().await;
        if let Some((redemption, _)) = &state.current_redeemer {
            info!("Stream went offline. Refunding last coin game redeemer: {}", redemption.user_name);
            self.refund_previous_redeemer(redemption).await?;
            state.current_redeemer = None;
        }
        state.is_active = false;
        state.current_price = state.default_price;
        Ok(())
    }
}

#[async_trait]
impl RedeemHandler for CoinGameAction {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        let mut state = self.state.write().await;

        if !state.is_active {
            return RedemptionResult {
                success: false,
                message: Some("The coin game is not currently active.".to_string()),
            };
        }

        let current_price = state.current_price;
        let price_multiplier = {
            let mut rng = self.rng.lock().await;
            rng.gen_range(1.5..=2.5)
        };
        let new_price = (current_price as f64 * price_multiplier).round() as u32;

        // Refund previous redeemer if exists
        if let Some((prev_redemption, _)) = &state.current_redeemer {
            if let Err(e) = self.refund_previous_redeemer(prev_redemption).await {
                error!("Failed to refund previous redeemer: {:?}", e);
                return RedemptionResult {
                    success: false,
                    message: Some("Failed to process the redemption. Please try again.".to_string()),
                };
            }
        }

        // Generate new message and update reward
        let new_message = self.generate_silly_message(&state.previous_redeemer, new_price).await;
        if let Err(e) = self.update_reward(&redemption.reward_id, new_price, &new_message).await {
            error!("Failed to update reward: {:?}", e);
            return RedemptionResult {
                success: false,
                message: Some("Failed to update the reward. Please try again.".to_string()),
            };
        }

        // Update state
        state.previous_redeemer = state.current_redeemer.take();
        state.current_redeemer = Some((redemption.clone(), current_price));
        state.current_price = new_price;

        RedemptionResult {
            success: true,
            message: Some(new_message),
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}