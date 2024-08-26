use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use futures_util::future::join_all;
use log::{debug, error, info, warn};
use tokio::sync::RwLock;
use tokio::time::timeout;
use crate::ai::AIClient;
use crate::osc::models::{OSCConfig, OSCMessageType, OSCValue};
use crate::osc::VRChatOSC;
use crate::twitch::api::TwitchAPIClient;
use crate::osc::osc_config::OSCConfigurations;
use crate::twitch::api::models::ChannelPointReward;
use crate::twitch::api::requests::channel_points;
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
        handlers.insert("Coin Game".to_string(), Box::new(CoinGameAction::new(coin_game_state.clone(), ai_client.clone(), api_client.clone())) as Box<dyn RedeemHandler>);
        handlers.insert("mao mao".to_string(), Box::new(AskAIAction::new(ai_client.clone())) as Box<dyn RedeemHandler>);
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
                match self.api_client.complete_channel_points(
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

    // You might want to add this method if you need cancellation handling
    pub async fn cancel_redemption(&self, redemption_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement cancellation logic here
        Ok(())
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
                cooldown: 0,
                is_global_cooldown: false,
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
                cooldown: 60,
                is_global_cooldown: false,
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
                reward_name: "Toss Pillow".to_string(),
                title: "Toss Pillow".to_string(),
                twitch_reward_id: None,
                cost: 50,
                prompt: "Toss a virtual pillow!".to_string(),
                cooldown: 0,
                is_global_cooldown: false,
                use_osc: true,
                osc_config: Some(OSCConfig {
                    uses_osc: true,
                    osc_endpoint: "/avatar/parameters/TossPillow".to_string(),
                    osc_type: OSCMessageType::Boolean,
                    osc_value: OSCValue::Boolean(true),
                    default_value: OSCValue::Boolean(false),
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
                reward_name: "custom_redeem".to_string(),
                title: "Custom Redeem".to_string(),
                twitch_reward_id: None,
                cost: 75,
                prompt: "This is a custom redeem".to_string(),
                cooldown: 0,
                is_global_cooldown: false,
                use_osc: false,
                osc_config: None,
                enabled_games: vec![],
                disabled_games: vec![],
                enabled_offline: false,
                user_input_required: false,
                is_active: false,
                auto_complete: true,
            },
        ];

        info!("Prepared {} redeems for initialization", redeems.len());

        let mut settings = self.redeem_settings.write().await;
        let mut osc_configs = self.osc_configs.write().await;

        for redeem in redeems {
            info!("Processing redeem: {}", redeem.title);
            settings.insert(redeem.reward_name.clone(), redeem.clone());

            if redeem.use_osc {
                if let Some(osc_config) = redeem.osc_config.clone() {
                    osc_configs.add_config(&redeem.title, osc_config);
                    info!("Added OSC config for {} with key {}", redeem.title, redeem.title);
                } else {
                    warn!("OSC is enabled for {} but no OSC config was provided", redeem.title);
                }
            }
        }

        info!("OSC configs after initialization: {:?}", osc_configs.configs.keys().collect::<Vec<_>>());

        drop(settings);
        drop(osc_configs);


        self.check_and_reset_coin_game().await?;

        match self.sync_configured_rewards().await {
            Ok(_) => info!("Successfully synced configured rewards"),
            Err(e) => error!("Failed to sync configured rewards: {:?}", e),
        }

        info!("Redeem initialization complete");
        Ok(())
    }

    pub async fn update_redeem(&self, settings: RedeemSettings) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement the logic to update a redeem on Twitch
        // Update self.redeem_settings with the new settings
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
        let mut state = self.coin_game_state.write().await;
        let broadcaster_id = self.api_client.get_broadcaster_id().await?;
        let is_live = self.api_client.is_stream_live(&broadcaster_id).await?;

        if state.is_active && is_live {
            // Reset price to default and refund any pending redeems
            state.current_price = state.default_price;
            if let Some((redemption, _)) = &state.current_redeemer {
                self.api_client.refund_channel_points(&redemption.reward_id, &redemption.id).await?;
            }
            state.current_redeemer = None;
            state.previous_redeemer = None;

            // Update the reward on Twitch
            let reward_id = self.get_coin_game_reward_id().await?;
            let initial_message = "The Coin Game has been reset! Who will be the first to join?";
            self.api_client.update_custom_reward(
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
        let mut settings = self.redeem_settings.write().await;
        let existing_redeems = self.api_client.get_channel_point_rewards().await?;
        let broadcaster_id = self.api_client.get_broadcaster_id().await?;

        for redeem_setting in settings.values_mut() {
            info!("Processing redeem: {}", redeem_setting.title);
            let should_be_active = self.should_redeem_be_active(redeem_setting).await;

            if should_be_active && redeem_setting.is_active {
                match existing_redeems.iter().find(|r| r.title == redeem_setting.title) {
                    Some(existing_reward) => {
                        // Update existing reward if needed
                        if existing_reward.cost != redeem_setting.cost ||
                            !existing_reward.is_enabled ||
                            existing_reward.is_user_input_required != redeem_setting.user_input_required ||
                            existing_reward.prompt != redeem_setting.prompt {
                            info!("Updating existing reward: {}", redeem_setting.title);
                            if let Err(e) = self.api_client.update_custom_reward(
                                &existing_reward.id,
                                &redeem_setting.title,
                                redeem_setting.cost,
                                true, // is_enabled
                                redeem_setting.cooldown,
                                &redeem_setting.prompt,
                                redeem_setting.user_input_required,
                            ).await {
                                error!("Failed to update reward {}: {}", redeem_setting.title, e);
                            } else {
                                redeem_setting.twitch_reward_id = Some(existing_reward.id.clone());
                            }
                        }
                    },
                    None => {
                        // Create new reward
                        info!("Creating new reward: {}", redeem_setting.title);
                        match self.api_client.create_custom_reward(
                            &redeem_setting.title,
                            redeem_setting.cost,
                            true, // is_enabled
                            redeem_setting.cooldown,
                            &redeem_setting.prompt,
                            redeem_setting.user_input_required,
                        ).await {
                            Ok(new_reward) => {
                                redeem_setting.twitch_reward_id = Some(new_reward.id);
                            },
                            Err(e) => error!("Failed to create reward {}: {}", redeem_setting.title, e),
                        }
                    }
                }
            } else {
                // Delete the reward if it exists (either it shouldn't be active or is_active is false)
                if let Some(existing_reward) = existing_redeems.iter().find(|r| r.title == redeem_setting.title) {
                    info!("Deleting inactive reward: {}", redeem_setting.title);
                    match self.api_client.delete_custom_reward(&broadcaster_id, &existing_reward.id).await {
                        Ok(_) => {
                            info!("Successfully deleted reward: {}", redeem_setting.title);
                            redeem_setting.twitch_reward_id = None;
                        },
                        Err(e) => {
                            if e.to_string().contains("404 Not Found") {
                                info!("Reward {} not found, possibly already deleted", redeem_setting.title);
                                redeem_setting.twitch_reward_id = None;
                            } else {
                                error!("Failed to delete reward {}: {}", redeem_setting.title, e);
                            }
                        }
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
        let settings = self.redeem_settings.read().await;
        let stream_status = self.stream_status.read().await;
        let broadcaster_id = self.api_client.get_broadcaster_id().await?;

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

            match channel_points::get_custom_reward(&self.api_client, &broadcaster_id, &redeem_setting.title).await {
                Ok(reward) => {
                    let reward_data = reward["data"][0].as_object().unwrap();
                    if reward_data["is_enabled"].as_bool().unwrap() != should_be_enabled {
                        info!("Updating redeem status on Twitch: {}. Enabled: {}", redeem_setting.title, should_be_enabled);
                        self.api_client.update_custom_reward(
                            reward_data["id"].as_str().unwrap(),
                            &redeem_setting.title,
                            redeem_setting.cost,
                            should_be_enabled,
                            redeem_setting.cooldown,
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

    pub async fn handle_stream_online(&self, game_name: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.coin_game_state.write().await;
        state.is_active = true;
        state.current_price = state.default_price;
        state.current_redeemer = None;
        state.previous_redeemer = None;

        let reward_id = self.get_coin_game_reward_id().await?;
        let initial_message = "The stream is live! The Coin Game has begun!";
        self.api_client.update_custom_reward(
            &reward_id,
            "Coin Game",
            state.default_price,
            true,
            0,
            initial_message,
            false,
        ).await?;

        let mut status = self.stream_status.write().await;
        status.is_live = true;
        status.current_game = game_name;
        drop(status);

        self.sync_configured_rewards().await
    }

    pub async fn handle_stream_offline(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.coin_game_state.write().await;

        // Refund the final redeemer if exists
        if let Some((redemption, _)) = &state.current_redeemer {
            self.api_client.refund_channel_points(&redemption.reward_id, &redemption.id).await?;
        }

        state.is_active = false;
        state.current_redeemer = None;
        state.previous_redeemer = None;

        let reward_id = self.get_coin_game_reward_id().await?;
        self.api_client.update_custom_reward(
            &reward_id,
            "Coin Game",
            state.default_price,
            false,  // Disable the reward
            0,
            "The Coin Game is currently inactive.",
            false,
        ).await?;

        let mut status = self.stream_status.write().await;
        status.is_live = false;
        status.current_game = String::new();
        drop(status);

        self.sync_configured_rewards().await
    }

    pub async fn handle_stream_update(&self, game_name: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut status = self.stream_status.write().await;
        status.current_game = game_name;
        drop(status);

        self.sync_configured_rewards().await
    }
}