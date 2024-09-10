
use serde::{Deserialize, Serialize};
use std::time::Duration;
use async_trait::async_trait;
use crate::osc::models as osc_models;

pub mod channel_points {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct ChannelPointReward {
        pub id: String,
        pub title: String,
        pub cost: u32,
        pub is_enabled: bool,
        pub is_paused: bool,
        pub is_in_stock: bool,
        pub is_user_input_required: bool,
        pub prompt: String,
        pub cooldown_seconds: Option<u32>,
        pub max_per_stream: Option<MaxPerStream>,
        pub max_per_user_per_stream: Option<MaxPerUserPerStream>,
        pub global_cooldown: Option<GlobalCooldown>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MaxPerStream {
        pub is_enabled: bool,
        pub max_per_stream: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct MaxPerUserPerStream {
        pub is_enabled: bool,
        pub max_per_user_per_stream: u32,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct GlobalCooldown {
        pub is_enabled: bool,
        pub global_cooldown_seconds: u32,
    }
}

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

#[derive(Debug, Clone)]
pub struct RedemptionResult {
    pub success: bool,
    pub message: Option<String>,
}

// #[derive(Debug, Clone, Serialize, Deserialize)]
// pub struct StreamStatus {
//     pub is_live: bool,
//     pub current_game: String,
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoinGameState {
    pub default_price: u32,
    pub current_price: u32,
    pub current_redeemer: Option<(Redemption, u32)>, // (Redemption, cost)
    pub previous_redeemer: Option<(Redemption, u32)>, // (Redemption, cost)
    pub is_active: bool,
}

impl CoinGameState {
    pub fn new(default_price: u32) -> Self {
        Self {
            default_price,
            current_price: default_price,
            current_redeemer: None,
            previous_redeemer: None,
            is_active: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RedeemSettings {
    pub reward_name: String,
    pub title: String,
    pub twitch_reward_id: Option<String>,
    pub cost: u32,
    pub prompt: String,
    pub cooldown: Option<u32>,
    pub is_global_cooldown: bool,
    pub limit_per_stream: Option<u32>,
    pub limit_per_user: Option<u32>,
    pub use_osc: bool,
    pub osc_config: Option<OSCConfig>,
    pub enabled_games: Vec<String>,
    pub disabled_games: Vec<String>,
    pub enabled_offline: bool,
    pub user_input_required: bool,
    pub is_active: bool,
    pub auto_complete: bool,
}

#[async_trait]
pub trait RedeemHandler: Send + Sync {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult;
}

impl RedeemSettings {
    pub fn validate_cooldown_settings(&self) -> Result<(), String> {
        if self.is_global_cooldown {
            if self.cooldown.is_none() && self.limit_per_stream.is_none() && self.limit_per_user.is_none() {
                return Err("When is_global_cooldown is true, at least one of cooldown, limit_per_stream, or limit_per_user must be set".to_string());
            }
        }
        Ok(())
    }

    pub fn get_cooldown_settings(&self) -> (Option<u32>, Option<u32>, Option<u32>) {
        (self.cooldown, self.limit_per_stream, self.limit_per_user)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OSCConfig {
    pub uses_osc: bool,
    pub osc_endpoint: String,
    pub osc_type: OSCMessageType,
    pub osc_value: OSCValue,
    pub default_value: OSCValue,
    pub execution_duration: Option<Duration>,
    pub send_chat_message: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OSCMessageType {
    Integer,
    Float,
    String,
    Bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OSCValue {
    Integer(i32),
    Float(f32),
    String(String),
    Bool(bool),
}

impl From<OSCConfig> for osc_models::OSCConfig {
    fn from(config: OSCConfig) -> Self {
        osc_models::OSCConfig {
            uses_osc: config.uses_osc,
            osc_endpoint: config.osc_endpoint,
            osc_type: config.osc_type.into(),
            osc_value: config.osc_value.into(),
            default_value: config.default_value.into(),
            execution_duration: config.execution_duration,
            send_chat_message: config.send_chat_message,
        }
    }
}

impl From<OSCMessageType> for osc_models::OSCMessageType {
    fn from(osc_type: OSCMessageType) -> Self {
        match osc_type {
            OSCMessageType::Integer => osc_models::OSCMessageType::Integer,
            OSCMessageType::Float => osc_models::OSCMessageType::Float,
            OSCMessageType::String => osc_models::OSCMessageType::String,
            OSCMessageType::Bool => osc_models::OSCMessageType::Boolean,
        }
    }
}

impl From<OSCValue> for osc_models::OSCValue {
    fn from(value: OSCValue) -> Self {
        match value {
            OSCValue::Integer(i) => osc_models::OSCValue::Integer(i),
            OSCValue::Float(f) => osc_models::OSCValue::Float(f),
            OSCValue::String(s) => osc_models::OSCValue::String(s),
            OSCValue::Bool(b) => osc_models::OSCValue::Boolean(b),
        }
    }
}

pub mod channel {
    #[derive(Debug, Clone)]
    pub struct Clip {
        pub title: String,
        pub url: String,
    }
}

pub mod followers {
    #[derive(Debug, Clone)]
    pub struct FollowerInfo {
        pub user_id: String,
        pub user_name: String,
        pub followed_at: String,
    }
}

pub mod shoutout {
    pub const GLOBAL_COOLDOWN_SECONDS: u64 = 121; // 2 minutes + 1 second buffer
    pub const USER_COOLDOWN_SECONDS: u64 = 3600; // 1 hour
}