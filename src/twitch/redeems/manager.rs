use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use log::{debug, error, info, warn};
use tokio::sync::{broadcast, RwLock};
use crate::ai::AIClient;
use crate::stream_state::StreamState;
use crate::twitch::api::requests::channel_points;
use crate::twitch::models::{Redemption, RedemptionResult, RedeemHandler, StreamStatus, CoinGameState, RedeemSettings, OSCConfig, OSCMessageType, OSCValue};
use crate::twitch::TwitchManager;
use super::actions::{CoinGameAction, AskAIAction, VRCOscRedeems};
use crate::stream_status::StreamStatusManager;

pub struct RedeemManager {
    twitch_manager: Arc<TwitchManager>,
    handlers: HashMap<String, Box<dyn RedeemHandler>>,
    coin_game_state: Arc<RwLock<CoinGameState>>,
    stream_status: Arc<RwLock<StreamStatus>>,
    redeem_settings: Arc<RwLock<HashMap<String, RedeemSettings>>>,
    stream_status_receiver: Option<broadcast::Receiver<bool>>,
}

impl Default for RedeemManager {
    fn default() -> Self {
        Self {
            twitch_manager: Arc::new(TwitchManager::default()),
            handlers: HashMap::new(),
            coin_game_state: Arc::new(RwLock::new(CoinGameState::new(20))),
            stream_status: Arc::new(RwLock::new(StreamStatus { is_live: false, current_game: String::new() })),
            redeem_settings: Arc::new(RwLock::new(HashMap::new())),
            stream_status_receiver: None,
        }
    }
}

struct VRCOscRedeemWrapper(Arc<VRCOscRedeems>);

#[async_trait]
impl RedeemHandler for VRCOscRedeemWrapper {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        self.0.handle(redemption).await
    }
}

impl RedeemManager {
    pub fn new(
        twitch_manager: Arc<TwitchManager>,
        ai_client: Arc<AIClient>,
    ) -> Self {
        let stream_status_receiver = Some(twitch_manager.stream_status_manager.subscribe());
        let coin_game_state = Arc::new(RwLock::new(CoinGameState::new(20)));
        let stream_status = Arc::new(RwLock::new(StreamStatus { is_live: false, current_game: String::new() }));
        let redeem_settings = Arc::new(RwLock::new(HashMap::new()));

        let vrc_osc_redeems = Arc::new(VRCOscRedeems::new(twitch_manager.clone()));

        let mut handlers = HashMap::new();
        handlers.insert("Coin Game".to_string(), Box::new(CoinGameAction::new(coin_game_state.clone(), ai_client.clone(), twitch_manager.get_api_client())) as Box<dyn RedeemHandler>);
        handlers.insert("mao mao".to_string(), Box::new(AskAIAction::new(ai_client.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("toss pillo".to_string(), Box::new(VRCOscRedeemWrapper(vrc_osc_redeems.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("cream pie".to_string(), Box::new(VRCOscRedeemWrapper(vrc_osc_redeems.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("water balloon".to_string(), Box::new(VRCOscRedeemWrapper(vrc_osc_redeems.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("cat trap".to_string(), Box::new(VRCOscRedeemWrapper(vrc_osc_redeems.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("snowball".to_string(), Box::new(VRCOscRedeemWrapper(vrc_osc_redeems.clone())) as Box<dyn RedeemHandler>);

        let mut redeem_manager = Self {
            twitch_manager,
            handlers,
            coin_game_state,
            stream_status,
            redeem_settings,
            stream_status_receiver,
        };

        redeem_manager.start_stream_status_listener();

        redeem_manager
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Shutting down RedeemManager...");

        info!("Saving RedeemManager settings...");
        if let Err(e) = self.save_settings().await {
            warn!("Error saving RedeemManager settings: {:?}", e);
        }


        info!("RedeemManager shutdown complete.");
        Ok(())
    }

    async fn save_settings(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement logic to save redeem settings
        Ok(())
    }

    pub async fn handle_redemption(&self, redemption: &Redemption) -> RedemptionResult {
        let api_client = self.twitch_manager.get_api_client();
        debug!("Handling redemption for: {}", redemption.reward_title);
        let settings = self.redeem_settings.read().await;
        let result = if let Some(handler) = self.handlers.get(&redemption.reward_title) {
            handler.handle(redemption).await
        } else {
            RedemptionResult {
                success: false,
                message: Some(format!("No handler found for redemption: {}", redemption.reward_title)),
            }
        };

        if let Some(redeem_setting) = settings.get(&redemption.reward_title) {
            if redeem_setting.auto_complete {
                match api_client.complete_channel_points(
                    &redemption.broadcaster_id,
                    &redemption.reward_id,
                    &redemption.id
                ).await {
                    Ok(_) => {
                        debug!("Auto-completed redemption: {}", redemption.reward_title);
                        if !result.success {
                            warn!("Redemption was auto-completed despite handler failure: {}", redemption.reward_title);
                        }
                    }
                    Err(e) => {
                        error!("Failed to auto-complete redemption {}: {:?}", redemption.reward_title, e);
                    }
                }
            }
        }

        result
    }

    pub async fn initialize_redeems(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting to initialize redeems");

        let redeems = vec![
            RedeemSettings {
                reward_name: "coin_game".to_string(),
                title: "Coin Game".to_string(),
                twitch_reward_id: None,
                cost: 20,
                prompt: "Enter the coin game! The price changes with each redemption.".to_string(),
                is_global_cooldown: false,
                limit_per_stream: None,
                limit_per_user: None,
                cooldown: Some(0),
                use_osc: false,
                osc_config: None,
                enabled_games: vec![],
                disabled_games: vec![],
                enabled_offline: false,
                user_input_required: false,
                is_active: true,
                auto_complete: false,
            },
            RedeemSettings {
                reward_name: "mao_mao".to_string(),
                title: "mao mao".to_string(),
                twitch_reward_id: None,
                cost: 555,
                prompt: "mao?".to_string(),
                is_global_cooldown: false,
                limit_per_stream: None,
                limit_per_user: None,
                cooldown: Some(60),
                use_osc: false,
                osc_config: None,
                enabled_games: vec![],
                disabled_games: vec![],
                enabled_offline: true,
                user_input_required: true,
                is_active: true,
                auto_complete: true,
            },
            RedeemSettings {
                reward_name: "toss pillo".to_string(),
                title: "toss pillo".to_string(),
                twitch_reward_id: None,
                cost: 48,
                prompt: "Toss a virtual pillow!".to_string(),
                is_global_cooldown: false,
                limit_per_stream: None,
                limit_per_user: None,
                cooldown: Some(0),
                use_osc: true,
                osc_config: Some(OSCConfig {
                    uses_osc: true,
                    osc_endpoint: "/avatar/parameters/twitch".to_string(),
                    osc_type: OSCMessageType::Integer,
                    osc_value: OSCValue::Integer(3),
                    default_value: OSCValue::Integer(0),
                    execution_duration: Some(Duration::from_secs(5)),
                    send_chat_message: false,
                }),
                enabled_games: vec!["VRChat".to_string()],
                disabled_games: vec![],
                enabled_offline: true,
                user_input_required: false,
                is_active: true,
                auto_complete: true,
            },
            RedeemSettings {
                reward_name: "cream_pie".to_string(),
                title: "cream pie".to_string(),
                twitch_reward_id: None,
                cost: 50,
                prompt: "Throw a virtual cream pie!".to_string(),
                is_global_cooldown: false,
                limit_per_stream: None,
                limit_per_user: None,
                cooldown: Some(0),
                use_osc: true,
                osc_config: Some(OSCConfig {
                    uses_osc: true,
                    osc_endpoint: "/avatar/parameters/twitch".to_string(),
                    osc_type: OSCMessageType::Integer,
                    osc_value: OSCValue::Integer(4),
                    default_value: OSCValue::Integer(0),
                    execution_duration: Some(Duration::from_secs(5)),
                    send_chat_message: false,
                }),
                enabled_games: vec!["VRChat".to_string()],
                disabled_games: vec![],
                enabled_offline: true,
                user_input_required: false,
                is_active: false,
                auto_complete: true,
            },
            RedeemSettings {
                reward_name: "water_balloon".to_string(),
                title: "water balloon".to_string(),
                twitch_reward_id: None,
                cost: 40,
                prompt: "Splash with a virtual water balloon!".to_string(),
                is_global_cooldown: false,
                limit_per_stream: None,
                limit_per_user: None,
                cooldown: Some(0),
                use_osc: true,
                osc_config: Some(OSCConfig {
                    uses_osc: true,
                    osc_endpoint: "/avatar/parameters/twitch".to_string(),
                    osc_type: OSCMessageType::Integer,
                    osc_value: OSCValue::Integer(5),
                    default_value: OSCValue::Integer(0),
                    execution_duration: Some(Duration::from_secs(5)),
                    send_chat_message: false,
                }),
                enabled_games: vec!["VRChat".to_string()],
                disabled_games: vec![],
                enabled_offline: true,
                user_input_required: false,
                is_active: false,
                auto_complete: true,
            },
            RedeemSettings {
                reward_name: "cat_trap".to_string(),
                title: "cat trap".to_string(),
                twitch_reward_id: None,
                cost: 840,
                prompt: "Deploy a virtual cat trap!".to_string(),
                is_global_cooldown: false,
                limit_per_stream: None,
                limit_per_user: None,
                cooldown: Some(60),
                use_osc: true,
                osc_config: Some(OSCConfig {
                    uses_osc: true,
                    osc_endpoint: "/avatar/parameters/twitch".to_string(),
                    osc_type: OSCMessageType::Integer,
                    osc_value: OSCValue::Integer(6),
                    default_value: OSCValue::Integer(0),
                    execution_duration: Some(Duration::from_secs(5)),
                    send_chat_message: false,
                }),
                enabled_games: vec!["VRChat".to_string()],
                disabled_games: vec![],
                enabled_offline: true,
                user_input_required: false,
                is_active: true,
                auto_complete: true,
            },
            RedeemSettings {
                reward_name: "snowball".to_string(),
                title: "snowball".to_string(),
                twitch_reward_id: None,
                cost: 45,
                prompt: "Throw a virtual snowball!".to_string(),
                is_global_cooldown: false,
                limit_per_stream: None,
                limit_per_user: None,
                cooldown: Some(0),
                use_osc: true,
                osc_config: Some(OSCConfig {
                    uses_osc: true,
                    osc_endpoint: "/avatar/parameters/twitch".to_string(),
                    osc_type: OSCMessageType::Integer,
                    osc_value: OSCValue::Integer(7),
                    default_value: OSCValue::Integer(0),
                    execution_duration: Some(Duration::from_secs(5)),
                    send_chat_message: false,
                }),
                enabled_games: vec!["VRChat".to_string()],
                disabled_games: vec![],
                enabled_offline: true,
                user_input_required: false,
                is_active: false,
                auto_complete: true,
            },
        ];

        info!("Prepared {} redeems for initialization", redeems.len());

        let mut settings = self.redeem_settings.write().await;
        let osc_configs = self.twitch_manager.get_osc_configs();
        for redeem in redeems {
            info!("Processing redeem: {}", redeem.title);
            settings.insert(redeem.reward_name.clone(), redeem.clone());

            if redeem.use_osc {
                if let Some(osc_config) = redeem.osc_config.clone() {
                    let mut configs = osc_configs.write().await;
                    configs.add_config(&redeem.title, osc_config.into());
                    info!("Added OSC config for {} with key {}", redeem.title, redeem.title);
                } else {
                    warn!("OSC is enabled for {} but no OSC config was provided", redeem.title);
                }
            }
        }

        {
            let configs = osc_configs.read().await;
            info!("OSC configs after initialization: {:?}", configs.configs.keys().collect::<Vec<_>>());
        }

        drop(settings);

        self.check_and_reset_coin_game().await?;

        match self.sync_configured_rewards().await {
            Ok(_) => info!("Successfully synced configured rewards"),
            Err(e) => error!("Failed to sync configured rewards: {:?}", e),
        }

        info!("Redeem initialization complete");
        Ok(())
    }

    pub async fn update_stream_status(&self, is_live: bool, current_game: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut status = self.stream_status.write().await;
        status.is_live = is_live;
        status.current_game = current_game.clone();
        drop(status);

        self.update_redeem_availabilities().await
    }

    async fn check_and_reset_coin_game(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let api_client = self.twitch_manager.get_api_client();
        let mut state = self.coin_game_state.write().await;
        let broadcaster_id = api_client.get_broadcaster_id().await?;
        let is_live = api_client.is_stream_live(&broadcaster_id).await?;

        if state.is_active && is_live {
            // Reset price to default and refund any pending redeems
            state.current_price = state.default_price;
            if let Some((redemption, _)) = &state.current_redeemer {
               api_client.refund_channel_points(&redemption.reward_id, &redemption.id).await?;
            }
            state.current_redeemer = None;
            state.previous_redeemer = None;

            // Update the reward on Twitch
            let reward_id = self.get_coin_game_reward_id().await?;
            let initial_message = "The Coin Game has been reset! Who will be the first to join?";
            api_client.update_custom_reward(
                &reward_id,
                "Coin Game",
                state.default_price,
                true,
                0,
                initial_message,
                false,
            ).await?;
        }

        Ok(())
    }

    async fn get_coin_game_reward_id(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // For now, just return the title of the reward
        Ok("Coin Game".to_string())
    }

    async fn sync_configured_rewards(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Syncing configured rewards");
        let api_client = self.twitch_manager.get_api_client();
        let settings = self.redeem_settings.read().await;
        let current_state = self.twitch_manager.stream_state_machine.get_current_state().await;
        let broadcaster_id = api_client.get_broadcaster_id().await?;

        for (_, redeem_setting) in settings.iter() {
            if !redeem_setting.is_active {
                info!("Skipping inactive redeem: {}", redeem_setting.title);
                continue;
            }

            let should_be_enabled = match current_state {
                StreamState::Live(ref game) => {
                    if !redeem_setting.disabled_games.is_empty() {
                        !redeem_setting.disabled_games.contains(game)
                    } else if !redeem_setting.enabled_games.is_empty() {
                        redeem_setting.enabled_games.contains(game)
                    } else {
                        true
                    }
                },
                StreamState::Offline => redeem_setting.enabled_offline,
                _ => false, // Disable during transitional states
            };

            // Update the reward on Twitch
            match api_client.get_custom_reward(&redeem_setting.title).await {
                Ok(existing_reward) => {
                    if existing_reward.is_enabled != should_be_enabled ||
                        existing_reward.cost != redeem_setting.cost ||
                        existing_reward.prompt != redeem_setting.prompt {
                        api_client.update_custom_reward(
                            &existing_reward.id,
                            &redeem_setting.title,
                            redeem_setting.cost,
                            should_be_enabled,
                            redeem_setting.cooldown.unwrap_or(0),
                            &redeem_setting.prompt,
                            redeem_setting.user_input_required,
                        ).await?;
                    }
                },
                Err(_) => {
                    if should_be_enabled {
                        api_client.create_custom_reward(
                            &redeem_setting.title,
                            redeem_setting.cost,
                            true,
                            redeem_setting.cooldown.unwrap_or(0),
                            &redeem_setting.prompt,
                            redeem_setting.user_input_required,
                        ).await?;
                    }
                }
            }
        }

        info!("Finished syncing configured rewards");
        Ok(())
    }

    async fn should_redeem_be_active(&self, redeem_setting: &RedeemSettings) -> bool {
        if !redeem_setting.is_active {
            return false;
        }

        let stream_status = self.stream_status.read().await;
        if stream_status.is_live {
            if !redeem_setting.disabled_games.is_empty() {
                !redeem_setting.disabled_games.contains(&stream_status.current_game)
            } else if !redeem_setting.enabled_games.is_empty() {
                redeem_setting.enabled_games.contains(&stream_status.current_game)
            } else {
                true
            }
        } else {
            redeem_setting.enabled_offline
        }
    }

    pub async fn handle_channel_status_update(&self, is_live: bool, current_game: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling channel status update. Is live: {}, Current game: {}", is_live, current_game);
        let mut status = self.stream_status.write().await;
        status.is_live = is_live;
        status.current_game = current_game.clone();
        drop(status);

        self.update_redeem_availabilities().await
    }

    async fn update_redeem_availabilities(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Updating redeem availabilities");
        let api_client = self.twitch_manager.get_api_client();
        let settings = self.redeem_settings.read().await;
        let stream_status = self.stream_status.read().await;
        let broadcaster_id = api_client.get_broadcaster_id().await?;

        for (_, redeem_setting) in settings.iter() {
            if !redeem_setting.is_active {
                info!("Skipping inactive redeem: {}", redeem_setting.title);
                continue;
            }

            let should_be_enabled = if stream_status.is_live {
                if !redeem_setting.disabled_games.is_empty() {
                    !redeem_setting.disabled_games.contains(&stream_status.current_game)
                } else if !redeem_setting.enabled_games.is_empty() {
                    redeem_setting.enabled_games.contains(&stream_status.current_game)
                } else {
                    true
                }
            } else {
                redeem_setting.enabled_offline
            };

            match channel_points::get_custom_reward(&api_client, &broadcaster_id, &redeem_setting.title).await {
                Ok(reward) => {
                    let reward_data = reward["data"][0].as_object().unwrap();
                    if reward_data["is_enabled"].as_bool().unwrap() != should_be_enabled {
                        info!("Updating redeem status on Twitch: {}. Enabled: {}", redeem_setting.title, should_be_enabled);
                        api_client.update_custom_reward(
                            reward_data["id"].as_str().unwrap(),
                            &redeem_setting.title,
                            redeem_setting.cost,
                            should_be_enabled,
                            redeem_setting.cooldown.unwrap_or(0),
                            &redeem_setting.prompt,
                            redeem_setting.user_input_required,
                        ).await?;
                    }
                },
                Err(e) => {
                    warn!("Failed to get custom reward for {}: {:?}", redeem_setting.title, e);
                    // If the reward doesn't exist, we don't need to do anything as it will be created in the next sync
                }
            }
        }

        info!("Finished updating redeem availabilities");
        Ok(())
    }

    fn start_stream_status_listener(&mut self) {
        if let Some(receiver) = self.stream_status_receiver.take() {
            let twitch_manager = self.twitch_manager.clone();
            tokio::spawn(async move {
                let mut receiver = receiver;
                while let Ok(is_live) = receiver.recv().await {
                    if let Err(e) = twitch_manager.handle_stream_status_change(is_live).await {
                        error!("Failed to handle stream status change: {}", e);
                    }
                }
            });
        }
    }

    async fn handle_stream_status_change(&mut self, is_live: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if is_live {
            self.handle_stream_online("".to_string()).await
        } else {
            self.handle_stream_offline().await
        }
    }


    pub async fn handle_stream_online(&self, game_name: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling stream online event. Game: {}", game_name);

        // Update stream state
        self.twitch_manager.stream_state_machine.set_stream_live(game_name.clone()).await?;

        let api_client = self.twitch_manager.get_api_client();
        let mut state = self.coin_game_state.write().await;
        state.is_active = true;
        state.current_price = state.default_price;
        state.current_redeemer = None;
        state.previous_redeemer = None;

        let reward_id = self.get_coin_game_reward_id().await?;
        let initial_message = "The stream is live! The Coin Game has begun!";
        api_client.update_custom_reward(
            &reward_id,
            "Coin Game",
            state.default_price,
            true,
            0,
            initial_message,
            false,
        ).await?;

        drop(state);

        self.sync_configured_rewards().await?;

        info!("Stream online handling complete");
        Ok(())
    }

    pub async fn handle_stream_offline(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling stream offline event");

        // Update stream state
        self.twitch_manager.stream_state_machine.set_stream_offline().await?;

        let api_client = self.twitch_manager.get_api_client();
        let mut state = self.coin_game_state.write().await;

        // Refund the final redeemer if exists
        if let Some((redemption, _)) = &state.current_redeemer {
            api_client.refund_channel_points(&redemption.reward_id, &redemption.id).await?;
        }

        state.is_active = false;
        state.current_redeemer = None;
        state.previous_redeemer = None;

        let reward_id = self.get_coin_game_reward_id().await?;
        api_client.update_custom_reward(
            &reward_id,
            "Coin Game",
            state.default_price,
            false,  // Disable the reward
            0,
            "The Coin Game is currently inactive.",
            false,
        ).await?;

        drop(state);

        self.sync_configured_rewards().await?;

        info!("Stream offline handling complete");
        Ok(())
    }

    pub async fn handle_stream_update(&self, game_name: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Handling stream update event. New game: {}", game_name);

        // Update stream state
        self.twitch_manager.stream_state_machine.update_game(game_name).await?;

        self.sync_configured_rewards().await?;

        info!("Stream update handling complete");
        Ok(())
    }
}