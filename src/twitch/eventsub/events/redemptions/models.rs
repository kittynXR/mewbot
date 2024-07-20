// src/twitch/eventsub/events/redemptions/models.rs

use serde::{Deserialize, Serialize};

use std::fmt;

use std::sync::Arc;
// src/twitch/eventsub/events/redemptions/models.rs

// src/twitch/eventsub/events/redemptions/models.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Redemption {
    pub id: String,
    pub broadcaster_id: String,  // Add this line
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

// Add a new enum for redemption completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RedemptionCompletion {
    NotApplicable,
    Pending,
    Completed,
}

// Update RedemptionResult to include the queue number
#[derive(Debug, Clone)]
pub struct RedemptionResult {
    pub success: bool,
    pub message: Option<String>,
    pub queue_number: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

// impl fmt::Debug for RedemptionActionConfig {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         f.debug_struct("RedemptionActionConfig")
//             .field("action", &self.action)
//             .field("queued", &self.queued)
//             .field("announce_in_chat", &self.announce_in_chat)
//             .field("requires_manual_completion", &self.requires_manual_completion)
//             .finish()
//     }
// }

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum RedemptionActionType {
    AIResponse,
    OSCMessage,
    UpdateText,
    Refund,
    Custom(String),
}

// impl fmt::Debug for RedemptionActionType {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Self::AIResponse => write!(f, "AIResponse"),
//             Self::OSCMessage => write!(f, "OSCMessage"),
//             Self::UpdateText => write!(f, "UpdateText"),
//             Self::Refund => write!(f, "Refund"),
//             Self::Custom(s) => write!(f, "Custom({})", s),
//         }
//     }
// }

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

// In models.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedemptionSettings {
    pub reward_id: String,
    pub title: String,
    pub cost: u32,
    pub action_config: RedemptionActionConfig,
    pub active: bool, // Add this line
}