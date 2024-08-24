use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::ai::AIClient;
use crate::osc::OSCClient;
use crate::twitch::api::TwitchAPIClient;
use super::models::{Redemption, RedemptionResult, RedemptionStatus, RedeemHandler, StreamStatus, CoinGameState};
use super::actions::{CoinGameAction, AskAIAction, TossPillowAction};

pub struct RedeemManager {
    api_client: Arc<TwitchAPIClient>,
    ai_client: Arc<AIClient>,
    osc_client: Arc<OSCClient>,
    handlers: HashMap<String, Box<dyn RedeemHandler>>,
    coin_game_state: Arc<RwLock<CoinGameState>>,
    stream_status: Arc<RwLock<StreamStatus>>,
}

impl RedeemManager {
    pub fn new(
        api_client: Arc<TwitchAPIClient>,
        ai_client: Arc<AIClient>,
        osc_client: Arc<OSCClient>,
    ) -> Self {
        let coin_game_state = Arc::new(RwLock::new(CoinGameState::new(20)));
        let stream_status = Arc::new(RwLock::new(StreamStatus { is_live: false, current_game: String::new() }));

        let mut handlers = HashMap::new();
        handlers.insert("Coin Game".to_string(), Box::new(CoinGameAction::new(coin_game_state.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("Ask AI".to_string(), Box::new(AskAIAction::new(ai_client.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("Toss Pillow".to_string(), Box::new(TossPillowAction::new(osc_client.clone())) as Box<dyn RedeemHandler>);

        Self {
            api_client,
            ai_client,
            osc_client,
            handlers,
            coin_game_state,
            stream_status,
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

    pub async fn update_stream_status(&self, is_live: bool, current_game: String) {
        let mut status = self.stream_status.write().await;
        status.is_live = is_live;
        status.current_game = current_game;
    }

    // Add more methods as needed, such as for initializing redeems, updating Twitch rewards, etc.
}