use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use log::debug;
use tokio::sync::RwLock;
use crate::twitch::redeems::models::{Redemption, RedemptionResult};
use crate::twitch::api::TwitchAPIClient;
use crate::ai::AIClient;
use crate::osc::VRChatOSC;
use crate::twitch::irc::client::TwitchIRCClientType;
use crate::twitch::redeems::RedeemManager;
use crate::twitch::redeems::actions::{handle_coin_game, osc_message};

#[async_trait]
pub trait RedeemAction: Send + Sync {
    async fn execute(&self, redemption: &Redemption, api_client: &TwitchAPIClient, irc_client: &Arc<TwitchIRCClientType>, channel: &str, ai_client: Option<&AIClient>, osc_client: Option<&VRChatOSC>, redeem_manager: &RedeemManager) -> RedemptionResult;
}

#[derive(Clone)]
pub struct DynamicActionManager {
    actions: Arc<RwLock<HashMap<String, Box<dyn RedeemAction>>>>,
}

impl DynamicActionManager {
    pub fn new() -> Self {
        let manager = Self {
            actions: Arc::new(RwLock::new(HashMap::new())),
        };

        let actions_clone = manager.actions.clone();

        tokio::spawn(async move {
            let mut actions = actions_clone.write().await;
            actions.insert("AI Response".to_string(), Box::new(AIResponseAction));
            actions.insert("AI Response with History".to_string(), Box::new(AIResponseWithHistoryAction));
            actions.insert("AI Response without History".to_string(), Box::new(AIResponseWithoutHistoryAction));
            actions.insert("OSC Message".to_string(), Box::new(OSCMessageAction));
            actions.insert("gamba time".to_string(), Box::new(CoinGameAction));
            // ... other default actions ...
        });

        manager
    }

    pub async fn register_action(&self, name: &str, action: Box<dyn RedeemAction>) {
        let mut actions = self.actions.write().await;
        actions.insert(name.to_string(), action);
    }

    pub async fn execute_action(&self, name: &str, redemption: &Redemption, api_client: &TwitchAPIClient, irc_client: &Arc<TwitchIRCClientType>, channel: &str, ai_client: Option<&AIClient>, osc_client: Option<&VRChatOSC>, redeem_manager: &RedeemManager) -> RedemptionResult {
        let actions = self.actions.read().await;
        if let Some(action) = actions.get(name) {
            action.execute(redemption, api_client, irc_client, channel, ai_client, osc_client, redeem_manager).await
        } else {
            RedemptionResult {
                success: false,
                message: Some(format!("Unknown action: {}", name)),
                queue_number: redemption.queue_number,
            }
        }
    }
}

// Update existing actions
pub struct AIResponseAction;
#[async_trait]
impl RedeemAction for AIResponseAction {
    async fn execute(&self, redemption: &Redemption, api_client: &TwitchAPIClient, irc_client: &Arc<TwitchIRCClientType>, channel: &str, ai_client: Option<&AIClient>, _osc_client: Option<&VRChatOSC>, redeem_manager: &RedeemManager) -> RedemptionResult {
        if let Some(ai_client) = ai_client {
            redeem_manager.handle_ai_response(redemption, ai_client).await
        } else {
            RedemptionResult {
                success: false,
                message: Some("AI client not initialized".to_string()),
                queue_number: redemption.queue_number,
            }
        }
    }
}

pub struct OSCMessageAction;

#[async_trait]
impl RedeemAction for OSCMessageAction {
    async fn execute(&self, redemption: &Redemption, _api_client: &TwitchAPIClient, _irc_client: &Arc<TwitchIRCClientType>, _channel: &str, _ai_client: Option<&AIClient>, osc_client: Option<&VRChatOSC>, redeem_manager: &RedeemManager) -> RedemptionResult {
        if let Some(osc_client) = osc_client {
            if let Some(settings) = redeem_manager.handlers_by_id.read().await.get(&redemption.reward_id) {
                if let Some(osc_config) = &settings.osc_config {
                    return osc_message::handle_osc_message(redemption, osc_client, osc_config).await;
                }
            }
        }
        RedemptionResult {
            success: false,
            message: Some("OSC client not initialized or OSC config not found".to_string()),
            queue_number: redemption.queue_number,
        }
    }
}

pub struct CoinGameAction;

#[async_trait]
impl RedeemAction for CoinGameAction {
    async fn execute(&self, redemption: &Redemption, api_client: &TwitchAPIClient, irc_client: &Arc<TwitchIRCClientType>, channel: &str, _ai_client: Option<&AIClient>, _osc_client: Option<&VRChatOSC>, redeem_manager: &RedeemManager) -> RedemptionResult {
        debug!("Executing CoinGameAction");
        handle_coin_game(redemption, api_client, irc_client, channel, redeem_manager).await
    }
}

pub struct AIResponseWithHistoryAction;
#[async_trait]
impl RedeemAction for AIResponseWithHistoryAction {
    async fn execute(&self, redemption: &Redemption, _api_client: &TwitchAPIClient, _irc_client: &Arc<TwitchIRCClientType>, _channel: &str, ai_client: Option<&AIClient>, _osc_client: Option<&VRChatOSC>, _redeem_manager: &RedeemManager) -> RedemptionResult {
        if let Some(ai_client) = ai_client {
            ai_client.generate_response_with_history(&redemption.user_input.clone().unwrap_or_default()).await
                .map_or_else(
                    |e| RedemptionResult {
                        success: false,
                        message: Some(format!("Failed to generate AI response: {}", e)),
                        queue_number: redemption.queue_number,
                    },
                    |response| RedemptionResult {
                        success: true,
                        message: Some(response),
                        queue_number: redemption.queue_number,
                    }
                )
        } else {
            RedemptionResult {
                success: false,
                message: Some("AI client not initialized".to_string()),
                queue_number: redemption.queue_number,
            }
        }
    }
}

pub struct AIResponseWithoutHistoryAction;
#[async_trait]
impl RedeemAction for AIResponseWithoutHistoryAction {
    async fn execute(&self, redemption: &Redemption, _api_client: &TwitchAPIClient, _irc_client: &Arc<TwitchIRCClientType>, _channel: &str, ai_client: Option<&AIClient>, _osc_client: Option<&VRChatOSC>, _redeem_manager: &RedeemManager) -> RedemptionResult {
        if let Some(ai_client) = ai_client {
            ai_client.generate_response_without_history(&redemption.user_input.clone().unwrap_or_default()).await
                .map_or_else(
                    |e| RedemptionResult {
                        success: false,
                        message: Some(format!("Failed to generate AI response: {}", e)),
                        queue_number: redemption.queue_number,
                    },
                    |response| RedemptionResult {
                        success: true,
                        message: Some(response),
                        queue_number: redemption.queue_number,
                    }
                )
        } else {
            RedemptionResult {
                success: false,
                message: Some("AI client not initialized".to_string()),
                queue_number: redemption.queue_number,
            }
        }
    }
}