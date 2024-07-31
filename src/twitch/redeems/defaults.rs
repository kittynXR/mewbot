use twitch_api::types::NamedUserColor::Red;
use super::models::{RedemptionSettings, RedemptionActionConfig, RedemptionActionType, OSCConfig};

pub fn get_default_redeems() -> Vec<RedemptionSettings> {
    vec![
        RedemptionSettings {
            reward_id: String::new(),
            title: "mao mao".to_string(),
            cost: 555,
            action_config: RedemptionActionConfig {
                action: RedemptionActionType::AIResponse,
                queued: true,
                announce_in_chat: true,
                requires_manual_completion: false,
            },
            active: true,
            cooldown: 120,
            prompt: "Ask Mao Mao anything!".to_string(),
            active_games: vec![], // Empty vec means active for all games
            offline_chat_redeem: true,
            osc_config: Some(OSCConfig {
                uses_osc: false,
                osc_endpoint: "".to_string(),
            })
        },
        RedemptionSettings {
            reward_id: String::new(),
            title: "coin game".to_string(),
            cost: 20,
            action_config: RedemptionActionConfig {
                action: RedemptionActionType::Custom("coin game".to_string()),
                queued: false,
                announce_in_chat: true,
                requires_manual_completion: false,
            },
            active: true,
            cooldown: 0,
            prompt: "Enter the coin game! The price changes with each redemption.".to_string(),
            active_games: vec![], // Empty vec means active for all games
            offline_chat_redeem: false,
            osc_config: Some(OSCConfig {
                uses_osc: false,
                osc_endpoint: "".to_string(),
            })
        },
        RedemptionSettings {
            reward_id: String::new(),
            title: "toss comfi pillo".to_string(),
            cost: 69,
            action_config: RedemptionActionConfig {
                action: RedemptionActionType::Custom("comfi pillo".to_string()),
                queued: false,
                announce_in_chat: true,
                requires_manual_completion: false,
            },
            active: false,
            cooldown: 0,
            prompt: "toss comfi pillo to see if stremer can catch it!".to_string(),
            active_games: vec!["VRChat".to_string()],
            offline_chat_redeem: true,
            osc_config: Some(OSCConfig {
                uses_osc: true,
                osc_endpoint: "/avatar/parameters/ComfiPillo".to_string(),
            }),
        },
    ]
}