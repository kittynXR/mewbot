// In coin_game.rs
use rand::{Rng, SeedableRng};
use std::sync::Arc;
use async_trait::async_trait;
use rand::rngs::StdRng;
use tokio::sync::{Mutex, RwLock};
use crate::ai::AIClient;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::redeems::models::{Redemption, RedemptionResult, RedeemHandler, CoinGameState};


pub struct CoinGameAction {
    state: Arc<RwLock<CoinGameState>>,
    ai_client: Arc<AIClient>,
    api_client: Arc<TwitchAPIClient>,
    rng: Arc<Mutex<StdRng>>, // Add this line
}

impl CoinGameAction {
    pub fn new(state: Arc<RwLock<CoinGameState>>, ai_client: Arc<AIClient>, api_client: Arc<TwitchAPIClient>) -> Self {
        Self {
            state,
            ai_client,
            api_client,
            rng: Arc::new(Mutex::new(StdRng::from_entropy())), // Initialize the RNG
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

        self.ai_client.generate_response_without_history(&prompt).await
            .unwrap_or_else(|_| format!("Coin game! New price: {} points", current_price))
    }

    async fn update_reward(&self, reward_id: &str, new_price: u32, new_prompt: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.api_client.update_custom_reward(
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
            if let Err(e) = self.api_client.refund_channel_points(
                &prev_redemption.reward_id,
                &prev_redemption.id,
            ).await {
                log::error!("Failed to refund previous redeemer: {:?}", e);
            }
        }

        // Generate new message and update reward
        let new_message = self.generate_silly_message(&state.previous_redeemer, new_price).await;
        if let Err(e) = self.update_reward("Coin Game", new_price, &new_message).await {
            log::error!("Failed to update reward: {:?}", e);
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
}