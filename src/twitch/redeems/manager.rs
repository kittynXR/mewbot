use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use log::info;
use tokio::sync::RwLock;
use crate::twitch::{TwitchAPIClient, TwitchManager};
use crate::ai::AIClient;
use crate::osc::OSCConfigurations;
use crate::twitch::models::{CoinGameState, RedeemConfigurations, RedeemHandler, RedeemInfo, Redemption, RedemptionResult};
use crate::twitch::redeems::{AskAIAction, SeriousAIAction, CoinGameAction, VRCOscRedeems};
use crate::twitch::redeems::registry::RedeemRegistry;
use crate::twitch::redeems::sync_manager::RedeemSyncManager;

pub struct RedeemManager {
    twitch_manager: Arc<TwitchManager>,
    ai_client: Arc<AIClient>,
    pub(crate) registry: Arc<RedeemRegistry>,
    sync_manager: Arc<RedeemSyncManager>,
    handlers: HashMap<String, Box<dyn RedeemHandler + Send + Sync>>,
    coin_game_state: Arc<RwLock<CoinGameState>>,
    osc_configs: Arc<RwLock<OSCConfigurations>>,  // Add this line
}

impl Clone for RedeemManager {
    fn clone(&self) -> Self {
        // Clone everything except handlers
        Self {
            twitch_manager: self.twitch_manager.clone(),
            ai_client: self.ai_client.clone(),
            registry: self.registry.clone(),
            sync_manager: self.sync_manager.clone(),
            handlers: HashMap::new(), // Start with an empty HashMap
            coin_game_state: self.coin_game_state.clone(),
            osc_configs: self.osc_configs.clone(),
        }
    }
}

impl RedeemManager {
    pub fn new(twitch_manager: Arc<TwitchManager>, ai_client: Arc<AIClient>) -> Self {
        let api_client = twitch_manager.get_api_client();
        let registry = Arc::new(RedeemRegistry::new());
        let sync_manager = Arc::new(RedeemSyncManager::new(api_client.clone()));

        let coin_game_state = Arc::new(RwLock::new(CoinGameState::new(20)));
        let vrc_osc_redeems = VRCOscRedeems::new(twitch_manager.clone());
        let osc_configs = twitch_manager.get_osc_configs();

        let mut redeem_manager = Self {
            twitch_manager: twitch_manager.clone(),
            ai_client: ai_client.clone(),
            registry,
            sync_manager,
            handlers: HashMap::new(),
            coin_game_state: coin_game_state.clone(),
            osc_configs,
        };

        let redeem_manager_arc = Arc::new(redeem_manager.clone());

        let mut handlers = HashMap::new();
        handlers.insert(
            "Coin Game".to_string(),
            Box::new(CoinGameAction::new(coin_game_state, redeem_manager_arc)) as Box<dyn RedeemHandler + Send + Sync>
        );
        handlers.insert(
            "mao mao".to_string(),
            Box::new(AskAIAction::new(ai_client.clone())) as Box<dyn RedeemHandler + Send + Sync>
        );
        handlers.insert(
            "get ai answer".to_string(),
            Box::new(SeriousAIAction::new(ai_client.clone())) as Box<dyn RedeemHandler + Send + Sync>
        );
        handlers.insert(
            "toss pillo".to_string(),
            Box::new(vrc_osc_redeems.clone()) as Box<dyn RedeemHandler + Send + Sync>
        );
        handlers.insert(
            "cream pie".to_string(),
            Box::new(vrc_osc_redeems.clone()) as Box<dyn RedeemHandler + Send + Sync>
        );
        handlers.insert(
            "water balloon".to_string(),
            Box::new(vrc_osc_redeems.clone()) as Box<dyn RedeemHandler + Send + Sync>
        );
        handlers.insert(
            "cat trap".to_string(),
            Box::new(vrc_osc_redeems.clone()) as Box<dyn RedeemHandler + Send + Sync>
        );
        handlers.insert(
            "snowball".to_string(),
            Box::new(vrc_osc_redeems) as Box<dyn RedeemHandler + Send + Sync>
        );

        redeem_manager.handlers = handlers;

        redeem_manager
    }

    pub async fn initialize_redeems(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let initial_configs = self.load_initial_configs().await?;
        let twitch_redeems = self.sync_manager.fetch_all_redeems().await?;

        for mut config in initial_configs {
            if !config.enabled_games.is_empty() && !config.disabled_games.is_empty() {
                log::warn!("Conflicting configuration for redeem '{}': both enabled_games and disabled_games are populated. Disabling this redeem.", config.title);
                config.is_conflicting = true;
                config.is_enabled = false;
            }

            let merged = if let Some(twitch_redeem) = twitch_redeems.iter().find(|r| r.title == config.title) {
                self.merge_redeem_info(config.clone(), twitch_redeem.clone())
            } else {
                config.clone()
            };

            // Add OSC configuration if present
            if merged.use_osc {
                if let Some(osc_config) = &merged.osc_config {
                    let mut osc_configs = self.osc_configs.write().await;
                    osc_configs.add_config(&merged.title, osc_config.clone().into());
                    println!("Added OSC config for {}: {:?}", merged.title, osc_config);
                }
            }

            self.registry.add_or_update(merged.title.clone(), merged).await;
        }

        // Print all OSC configs for debugging
        {
            let osc_configs = self.osc_configs.read().await;
            println!("All OSC configs after initialization: {:?}", osc_configs.configs.keys().collect::<Vec<_>>());
        }

        // Sync with Twitch
        self.sync_configured_rewards().await?;

        Ok(())
    }

    async fn load_initial_configs(&self) -> Result<Vec<RedeemInfo>, Box<dyn Error + Send + Sync>> {
        let config_path = "redeems_config.json";  // You might want to make this configurable
        let configs = RedeemConfigurations::load(config_path)?;
        Ok(configs.redeems)
    }

    fn merge_redeem_info(&self, local: RedeemInfo, twitch: RedeemInfo) -> RedeemInfo {
        RedeemInfo {
            id: twitch.id,
            title: local.title,
            cost: local.cost,
            is_enabled: local.is_enabled,
            prompt: local.prompt,
            cooldown: local.cooldown,
            is_global_cooldown: local.is_global_cooldown,
            limit_per_stream: local.limit_per_stream,
            limit_per_user: local.limit_per_user,
            use_osc: local.use_osc,
            osc_config: local.osc_config,
            enabled_games: local.enabled_games,
            disabled_games: local.disabled_games,
            enabled_offline: local.enabled_offline,
            is_conflicting: local.is_conflicting,
            user_input_required: local.user_input_required,
            auto_complete: local.auto_complete,
        }
    }

    pub async fn sync_configured_rewards(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let all_redeems = self.registry.get_all().await;
        let current_game = self.twitch_manager.get_current_game().await;
        let is_live = self.twitch_manager.is_stream_live().await;

        for redeem in all_redeems {
            let should_be_active = redeem.is_active(current_game.as_deref(), is_live);

            if should_be_active {
                if let Some(id) = redeem.id.as_ref() {
                    // Update existing redeem
                    self.sync_manager.update_redeem(&redeem).await?;
                } else {
                    // Create new redeem
                    let new_id = self.sync_manager.create_redeem(&redeem).await?;
                    let mut updated_redeem = redeem.clone();
                    updated_redeem.id = Some(new_id);
                    self.registry.add_or_update(redeem.title.clone(), updated_redeem).await;
                }
            } else if let Some(id) = redeem.id.as_ref() {
                // Delete inactive redeem
                self.sync_manager.delete_redeem(id).await?;
                let mut updated_redeem = redeem.clone();
                updated_redeem.id = None;
                self.registry.add_or_update(redeem.title.clone(), updated_redeem).await;
            }
        }

        Ok(())
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

    pub async fn handle_stream_online(&self, game: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling stream online event. Game: {}", game);

        // Update all redeems based on the new stream state
        self.sync_configured_rewards().await?;

        // Reset Coin Game state
        let mut coin_game_state = self.coin_game_state.write().await;
        coin_game_state.is_active = true;
        coin_game_state.current_price = coin_game_state.default_price;
        coin_game_state.current_redeemer = None;
        coin_game_state.previous_redeemer = None;

        Ok(())
    }

    pub async fn handle_stream_offline(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling stream offline event");

        // Update all redeems based on the new stream state
        self.sync_configured_rewards().await?;

        // Handle Coin Game offline state
        if let Some(coin_game_action) = self.handlers.get("Coin Game") {
            if let Some(coin_game) = coin_game_action.as_any().downcast_ref::<CoinGameAction>() {
                coin_game.handle_offline().await?;
            }
        }

        // Existing code to disable Coin Game
        let mut coin_game_state = self.coin_game_state.write().await;
        coin_game_state.is_active = false;

        Ok(())
    }

    pub async fn handle_stream_update(&self, game_name: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling stream update event. New game: {}", game_name);

        // Update all redeems based on the new game
        self.sync_configured_rewards().await?;

        Ok(())
    }

    pub fn get_api_client(&self) -> Arc<TwitchAPIClient> {
        self.twitch_manager.get_api_client()
    }

    pub fn get_ai_client(&self) -> Arc<AIClient> {
        self.ai_client.clone()
    }
}