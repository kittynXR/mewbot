use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::osc::models::OSCConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redemption {
    pub id: String,
    pub broadcaster_id: String,
    pub user_id: String,
    pub user_name: String,
    pub reward_id: String,
    pub reward_title: String,
    pub user_input: Option<String>,
    pub status: RedemptionStatus,
}

#[derive(Debug, Clone)]
pub struct RedemptionResult {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedemptionStatus {
    Unfulfilled,
    Fulfilled,
    Canceled,
}

impl From<&str> for RedemptionStatus {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "unfulfilled" => RedemptionStatus::Unfulfilled,
            "fulfilled" => RedemptionStatus::Fulfilled,
            "canceled" => RedemptionStatus::Canceled,
            _ => RedemptionStatus::Unfulfilled, // Default case
        }
    }
}

#[async_trait]
pub trait RedeemHandler: Send + Sync {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamStatus {
    pub is_live: bool,
    pub current_game: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinGameState {
    pub current_price: u32,
    pub last_redemption: Option<Redemption>,
}

impl CoinGameState {
    pub fn new(initial_price: u32) -> Self {
        Self {
            current_price: initial_price,
            last_redemption: None,
        }
    }
}

// Add this new struct to represent the redeem settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedeemSettings {
    pub reward_id: String,
    pub title: String,
    pub cost: u32,
    pub prompt: String,
    pub cooldown: u32,
    pub is_global_cooldown: bool,
    pub use_osc: bool,
    pub osc_config: Option<OSCConfig>,
}