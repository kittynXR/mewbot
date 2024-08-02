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
    pub queued: bool,
    pub queue_number: Option<usize>,
    pub announce_in_chat: bool,
}



#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RedemptionCompletion {
    NotApplicable,
    Pending,
    Completed,
}

#[derive(Debug, Clone)]
pub struct RedemptionResult {
    pub success: bool,
    pub message: Option<String>,
    pub queue_number: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RedemptionStatus {
    Unfulfilled,
    Fulfilled,
    Canceled,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct RedemptionActionConfig {
    pub action: RedemptionActionType,
    pub queued: bool,
    pub announce_in_chat: bool,
    pub requires_manual_completion: bool,
}



#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum RedemptionActionType {
    AIResponse,
    AIResponseWithHistory,
    AIResponseWithoutHistory,
    OSCMessage,
    UpdateText,
    Refund,
    Custom(String),
}

impl ToString for RedemptionActionType {
    fn to_string(&self) -> String {
        match self {
            RedemptionActionType::AIResponse => "AI Response".to_string(),
            RedemptionActionType::AIResponseWithHistory => "AI Response with History".to_string(),
            RedemptionActionType::AIResponseWithoutHistory => "AI Response without History".to_string(),
            RedemptionActionType::OSCMessage => "OSC Message".to_string(),
            RedemptionActionType::UpdateText => "Update Text".to_string(),
            RedemptionActionType::Refund => "Refund".to_string(),
            RedemptionActionType::Custom(name) => name.clone(),
        }
    }
}

pub trait RedemptionHandler {
    fn handle(&self, redemption: Redemption) -> RedemptionResult;
}

impl From<&str> for RedemptionStatus {
    fn from(s: &str) -> Self {
        match s {
            "UNFULFILLED" => RedemptionStatus::Unfulfilled,
            "FULFILLED" => RedemptionStatus::Fulfilled,
            "CANCELED" => RedemptionStatus::Canceled,
            _ => RedemptionStatus::Unfulfilled,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionSettings {
    pub reward_id: String,
    pub title: String,
    pub auto_complete: bool,
    pub cost: u32,
    pub action_config: RedemptionActionConfig,
    pub active: bool,
    pub cooldown: u32,
    pub prompt: String,
    pub active_games: Vec<String>,
    pub offline_chat_redeem: bool,
    pub osc_config: Option<OSCConfig>,
}

// Add this new struct for the coin game state
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