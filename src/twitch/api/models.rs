use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelPointReward {
    pub id: String,
    pub title: String,
    pub cost: u32,
    pub is_enabled: bool,
    pub is_paused: bool,
    pub is_in_stock: bool,
    pub is_user_input_required: bool,
    // Add other fields as needed
}