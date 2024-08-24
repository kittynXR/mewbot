use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::twitch::redeems::models::{Redemption, RedemptionResult, RedeemHandler, CoinGameState};

pub struct CoinGameAction {
    state: Arc<RwLock<CoinGameState>>,
}

impl CoinGameAction {
    pub fn new(state: Arc<RwLock<CoinGameState>>) -> Self {
        Self { state }
    }
}

#[async_trait]
impl RedeemHandler for CoinGameAction {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        let mut state = self.state.write().await;
        let current_price = state.current_price;
        let new_price = (current_price as f64 * (1.5 + rand::random::<f64>())).round() as u32;

        // Implement coin game logic here
        // For now, we'll just update the price and return a success message

        state.current_price = new_price;
        state.last_redemption = Some(redemption.clone());

        RedemptionResult {
            success: true,
            message: Some(format!("Coin game! New price: {} points", new_price)),
        }
    }
}