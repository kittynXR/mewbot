use twitch_api::types::NamedUserColor::Red;
use super::models::{RedemptionSettings, RedemptionActionConfig, RedemptionActionType};

pub fn get_default_redeems() -> Vec<RedemptionSettings> {
    vec![
        RedemptionSettings {
            reward_id: String::new(),
            title: "mao mao".to_string(),
            cost: 3,
            action_config: RedemptionActionConfig {
                action: RedemptionActionType::AIResponse,
                queued: true,
                announce_in_chat: true,
                requires_manual_completion: false,
            },
            active: true,
        },
        RedemptionSettings {
            reward_id: String::new(),
            title: "coin game".to_string(),
            cost: 20,  // Updated to the initial cost you mentioned
            action_config: RedemptionActionConfig {
                action: RedemptionActionType::Custom("coin game".to_string()),
                queued: false,
                announce_in_chat: true,
                requires_manual_completion: false,
            },
            active: true,
        },
    ]
}