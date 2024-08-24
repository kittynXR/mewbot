use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::ai::AIClient;
use crate::osc::VRChatOSC;
use crate::twitch::api::TwitchAPIClient;
use crate::osc::osc_config::OSCConfigurations;
use super::models::{Redemption, RedemptionResult, RedemptionStatus, RedeemHandler, StreamStatus, CoinGameState, RedeemSettings};
use super::actions::{CoinGameAction, AskAIAction, TossPillowAction};

pub struct RedeemManager {
    api_client: Arc<TwitchAPIClient>,
    ai_client: Arc<AIClient>,
    vrchat_osc: Arc<VRChatOSC>,
    handlers: HashMap<String, Box<dyn RedeemHandler>>,
    coin_game_state: Arc<RwLock<CoinGameState>>,
    stream_status: Arc<RwLock<StreamStatus>>,
    osc_configs: Arc<RwLock<OSCConfigurations>>,
    redeem_settings: Arc<RwLock<HashMap<String, RedeemSettings>>>,
}

impl RedeemManager {
    pub fn new(
        api_client: Arc<TwitchAPIClient>,
        ai_client: Arc<AIClient>,
        vrchat_osc: Arc<VRChatOSC>,
        osc_configs: Arc<RwLock<OSCConfigurations>>,
    ) -> Self {
        let coin_game_state = Arc::new(RwLock::new(CoinGameState::new(20)));
        let stream_status = Arc::new(RwLock::new(StreamStatus { is_live: false, current_game: String::new() }));
        let redeem_settings = Arc::new(RwLock::new(HashMap::new()));

        let mut handlers = HashMap::new();
        handlers.insert("Coin Game".to_string(), Box::new(CoinGameAction::new(coin_game_state.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("Ask AI".to_string(), Box::new(AskAIAction::new(ai_client.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("Toss Pillow".to_string(), Box::new(TossPillowAction::new(vrchat_osc.clone(), osc_configs.clone())) as Box<dyn RedeemHandler>);

        Self {
            api_client,
            ai_client,
            vrchat_osc,
            handlers,
            coin_game_state,
            stream_status,
            osc_configs,
            redeem_settings,
        }
    }

    pub async fn handle_redemption(&self, redemption: &Redemption) -> RedemptionResult {
        if let Some(handler) = self.handlers.get(&redemption.reward_title) {
            handler.handle(redemption).await
        } else {
            RedemptionResult {
                success: false,
                message: Some(format!("No handler found for redemption: {}", redemption.reward_title)),
            }
        }
    }

    // You might want to add this method if you need cancellation handling
    pub async fn cancel_redemption(&self, redemption_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement cancellation logic here
        Ok(())
    }

    pub async fn update_stream_status(&self, is_live: bool, current_game: String) {
        let mut status = self.stream_status.write().await;
        status.is_live = is_live;
        status.current_game = current_game;
    }

    pub async fn initialize_redeems(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement the logic to initialize redeems from Twitch API
        // Update self.redeem_settings with the fetched data
        Ok(())
    }

    pub async fn update_redeem(&self, settings: RedeemSettings) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement the logic to update a redeem on Twitch
        // Update self.redeem_settings with the new settings
        Ok(())
    }

    // Add more methods as needed
}