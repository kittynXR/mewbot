use super::models::{Redemption, RedemptionActionType, RedemptionActionConfig, RedemptionResult, RedemptionStatus, RedemptionSettings, CoinGameState};
use crate::ai::AIClient;
use crate::osc::VRChatOSC;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::api::requests::channel_points;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::collections::{HashMap, VecDeque, HashSet};
use std::sync::Arc;
use async_trait::async_trait;
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};
use tokio::sync::{Mutex, RwLock};
use serde_json;
use crate::twitch::api::models::ChannelPointReward;
use super::{actions, RedeemAction};
use super::dynamic_action_manager::{DynamicActionManager, AIResponseAction, OSCMessageAction, CoinGameAction};
use crate::twitch::irc::client::TwitchIRCClientType;
use super::actions::ai_response::{AIResponseManager, AIResponseConfig, AIProvider, AIResponseType};
use crate::twitch::redeems::actions::handle_custom_action;

pub struct RedeemManager {
    pub(crate) handlers_by_id: Arc<RwLock<HashMap<String, RedemptionSettings>>>,
    pub(crate) handlers_by_name: Arc<RwLock<HashMap<String, RedemptionSettings>>>,
    pub(crate) ai_client: Option<Arc<AIClient>>,
    chat_history: Arc<RwLock<String>>,
    pub(crate) osc_client: Option<Arc<VRChatOSC>>,
    pub(crate) api_client: Arc<TwitchAPIClient>,
    queue: Mutex<VecDeque<Option<Redemption>>>,
    next_queue_number: Mutex<usize>,
    settings_file: String,
    redemption_results: RwLock<HashMap<String, RedemptionResult>>,
    reward_configs: RwLock<HashMap<String, RedemptionActionConfig>>,
    action_manager: DynamicActionManager,
    pub(crate) ai_response_manager: AIResponseManager,
    pub(crate) coin_game_state: Arc<RwLock<CoinGameState>>,
    pub(crate) processed_redemptions: Mutex<HashSet<String>>,
    stream_status: Arc<RwLock<StreamStatus>>,
}

pub struct StreamStatus {
    pub is_live: bool,
    pub current_game: String,
}

impl StreamStatus {
    pub fn new(is_live: bool, current_game: String) -> Self {
        Self {
            is_live,
            current_game,
        }
    }
}

impl RedeemManager {
    pub fn new(
        ai_client: Option<Arc<AIClient>>,
        osc_client: Option<Arc<VRChatOSC>>,
        api_client: Arc<TwitchAPIClient>,
    ) -> Self {
        let mut reward_configs = HashMap::new();
        let mut ai_response_manager = AIResponseManager::new();
        ai_response_manager.initialize_ai_responses();

        // Initialize default AI response configs
        ai_response_manager.add_config(
            "mao mao".to_string(),
            AIResponseConfig {
                provider: AIProvider::OpenAI,
                model: "gpt-4o-mini".to_string(),
                prompt: "You are an entertaining chatbot in a cute and funny catgirl named kittyn's twitch channel".to_string(),
                max_tokens: 100,
                temperature: 0.7,
                response_type: AIResponseType::WithHistory,
            }
        );

        reward_configs.insert("AI Response".to_string(), RedemptionActionConfig {
            action: RedemptionActionType::AIResponse,
            queued: true,
            announce_in_chat: true,
            requires_manual_completion: false,
        });

        let action_manager = DynamicActionManager::new();

        let manager = Self {
            handlers_by_id: Arc::new(RwLock::new(HashMap::new())),
            handlers_by_name: Arc::new(RwLock::new(HashMap::new())),
            ai_client,
            chat_history: Arc::new(RwLock::new(String::new())),
            osc_client,
            api_client,
            queue: Mutex::new(VecDeque::new()),
            next_queue_number: Mutex::new(1),
            settings_file: "redemption_settings.json".to_string(),
            redemption_results: RwLock::new(HashMap::new()),
            reward_configs: RwLock::new(reward_configs),
            action_manager,
            ai_response_manager,
            coin_game_state: Arc::new(RwLock::new(CoinGameState::new(20))),
            processed_redemptions: Mutex::new(HashSet::new()),
            stream_status: Arc::new(RwLock::new(StreamStatus::new(false, "".to_string()))),
        };

        // Register default actions
        let action_manager_clone = manager.action_manager.clone();
        tokio::spawn(async move {
            action_manager_clone.register_action("AI Response", Box::new(AIResponseAction)).await;
            action_manager_clone.register_action("OSC Message", Box::new(OSCMessageAction)).await;
            action_manager_clone.register_action("coin game", Box::new(CoinGameAction)).await;
        });

        manager
    }

    pub async fn initialize_redeems(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting initialize_redeems method");
        let settings = self.load_settings().await?;
        let existing_rewards = self.api_client.get_channel_point_rewards().await?;

        println!("Initializing redeems. Settings: {:?}", settings);

        let mut handlers_by_id = self.handlers_by_id.write().await;
        let mut handlers_by_name = self.handlers_by_name.write().await;
        println!("Initializing handlers. Current count: {}", handlers_by_id.len());

        let stream_status = self.stream_status.read().await;
        let is_live = stream_status.is_live;
        drop(stream_status);

        for setting in settings {
            let existing_reward = existing_rewards.iter().find(|r| r.title == setting.title);

            let updated_setting = match existing_reward {
                Some(reward) if reward.is_user_input_required == setting.action_config.queued => {

                    let should_be_active = if is_live {
                        setting.active
                    } else {
                        setting.offline_chat_redeem
                    };

                    println!("Updating existing reward: {} (ID: {})", setting.title, reward.id);
                    match self.api_client.update_custom_reward(
                        &reward.id,
                        &setting.title,
                        setting.cost,
                        should_be_active,  // Use should_be_active instead of setting.active
                        setting.cooldown,
                        &setting.prompt
                    ).await {
                        Ok(_) => {
                            println!("Successfully updated reward: {}", setting.title);
                            let mut updated = setting;
                            updated.reward_id = reward.id.clone();
                            updated
                        },
                        Err(e) => {
                            eprintln!("Failed to update reward {}: {}", setting.title, e);
                            continue;
                        }
                    }
                }
                _ => {
                    println!("Creating new reward: {}", setting.title);
                    match self.api_client.create_custom_reward(
                        &setting.title,
                        setting.cost,
                        setting.action_config.queued,
                        setting.cooldown,
                        &setting.prompt
                    ).await {
                        Ok(new_reward) => {
                            println!("Successfully created reward: {} (ID: {})", setting.title, new_reward.id);
                            let mut new_setting = setting;
                            new_setting.reward_id = new_reward.id.clone();
                            new_setting
                        },
                        Err(e) => {
                            eprintln!("Failed to create reward {}: {}", setting.title, e);
                            continue;
                        }
                    }
                }
            };

            println!("Registering handler for reward: {} (ID: {})", updated_setting.title, updated_setting.reward_id);
            handlers_by_id.insert(updated_setting.reward_id.clone(), updated_setting.clone());
            handlers_by_name.insert(updated_setting.title.clone(), updated_setting.clone());


            // Register the action with the DynamicActionManager
            match &updated_setting.action_config.action {
                RedemptionActionType::AIResponse => {
                    self.action_manager.register_action(&updated_setting.title, Box::new(AIResponseAction)).await;
                }
                RedemptionActionType::OSCMessage => {
                    self.action_manager.register_action(&updated_setting.title, Box::new(OSCMessageAction)).await;
                }
                RedemptionActionType::Custom(name) if name == "coin game" => {
                    self.action_manager.register_action(&updated_setting.title, Box::new(CoinGameAction)).await;
                }
                _ => {
                    println!("Unknown action type for reward: {}", updated_setting.title);
                }
            }
        }

        // Ensure the coin game reward is registered
        if !handlers_by_name.contains_key("coin game") {
            let coin_game_setting = RedemptionSettings {
                reward_id: String::new(), // This will be filled in when created
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
                active_games: vec![],
                offline_chat_redeem: false, // Add this line
            };

            match self.api_client.create_custom_reward(
                &coin_game_setting.title,
                coin_game_setting.cost,
                coin_game_setting.action_config.queued,
                coin_game_setting.cooldown,
                &coin_game_setting.prompt
            ).await {
                Ok(new_reward) => {
                    println!("Successfully created coin game reward: {} (ID: {})", coin_game_setting.title, new_reward.id);
                    let mut new_setting = coin_game_setting;
                    new_setting.reward_id = new_reward.id.clone();
                    handlers_by_id.insert(new_setting.reward_id.clone(), new_setting.clone());
                    handlers_by_name.insert(new_setting.title.clone(), new_setting.clone());
                    self.action_manager.register_action(&new_setting.title, Box::new(CoinGameAction)).await;
                },
                Err(e) => {
                    eprintln!("Failed to create coin game reward: {}", e);
                }
            }
        }

        println!("Redeems initialization complete. Registered handlers: {:?}", handlers_by_id);
        Ok(())
    }

    pub async fn handle_redemption(&self, redemption: Redemption, irc_client: Arc<TwitchIRCClientType>, channel: String) -> RedemptionResult {
        let mut processed = self.processed_redemptions.lock().await;
        if processed.contains(&redemption.id) {
            println!("Skipping already processed redemption: {:?}", redemption);
            return RedemptionResult {
                success: true,
                message: Some("Redemption already processed".to_string()),
                queue_number: None,
            };
        }
        processed.insert(redemption.id.clone());
        drop(processed);
        println!("Handling redemption: {:?}", redemption);

        let handlers_by_id = self.handlers_by_id.read().await;
        let handlers_by_name = self.handlers_by_name.read().await;

        let settings = handlers_by_id.get(&redemption.reward_id)
            .or_else(|| handlers_by_name.get(&redemption.reward_title));

        if let Some(settings) = settings {
            println!("Found handler for reward: {:?}", settings);

            let result = if settings.title == "coin game" {
                println!("Processing coin game redemption");
                self.handle_coin_game(&redemption, &irc_client, &channel).await
            } else {
                println!("Processing regular redemption");
                self.execute_action(&redemption, &settings.action_config, &irc_client, &channel).await
            };

            // Only update redemption status for non-coin game redeems
            if settings.title != "coin game" {
                let status = if result.success {
                    RedemptionStatus::Fulfilled
                } else {
                    RedemptionStatus::Canceled
                };
                self.update_redemption_status(&redemption, status).await;
            }

            // Send the response to chat
            if let Some(message) = &result.message {
                let chat_message = format!("@{}: {}", redemption.user_name, message);
                if let Err(e) = irc_client.say(channel, chat_message).await {
                    eprintln!("Failed to send message to chat: {}", e);
                }
            }

            result
        } else {
            println!("No handler found for reward ID: {} or name: {}", redemption.reward_id, redemption.reward_title);
            RedemptionResult {
                success: false,
                message: Some(format!("No handler registered for reward ID: {} or name: {}", redemption.reward_id, redemption.reward_title)),
                queue_number: None,
            }
        }
    }

    pub async fn manually_update_redemption_status(&self, redemption_id: &str, status: RedemptionStatus) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let status_str = match status {
            RedemptionStatus::Unfulfilled => "UNFULFILLED",
            RedemptionStatus::Fulfilled => "FULFILLED",
            RedemptionStatus::Canceled => "CANCELED",
        };

        self.api_client.update_redemption_status("", redemption_id, status_str).await?;
        Ok(())
    }

    pub async fn handle_coin_game(&self, redemption: &Redemption, irc_client: &Arc<TwitchIRCClientType>, channel: &str) -> RedemptionResult {
        println!("Executing CoinGameAction for redemption: {:?}", redemption);

        let mut state = self.coin_game_state.write().await;
        let current_price = state.current_price;
        let new_price = (current_price as f64 * (1.5 + rand::random::<f64>())).round() as u32;

        if let Some(previous_redemption) = state.last_redemption.take() {
            // Refund the previous redemption
            if let Err(e) = self.api_client.refund_channel_points(&previous_redemption.reward_id, &previous_redemption.id).await {
                eprintln!("Failed to refund previous coin game redemption: {}", e);
            } else {
                let refund_message = format!(
                    "{} is cute!",
                    previous_redemption.user_name
                );
                if let Err(e) = irc_client.say(channel.to_string(), refund_message).await {
                    eprintln!("Failed to send refund message to chat: {}", e);
                }
            }
        }

        let handlers_by_id = self.handlers_by_id.read().await;
        let settings = match handlers_by_id.get(&redemption.reward_id) {
            Some(s) => s,
            None => {
                eprintln!("No handler found for reward ID: {}", redemption.reward_id);
                return RedemptionResult {
                    success: false,
                    message: Some("Failed to process coin game: reward not found".to_string()),
                    queue_number: redemption.queue_number,
                };
            }
        };

        // Generate AI message
        let ai_prompt = format!("Create a short, fun message (max 50 characters) about {} entering the coin game on twitch.", redemption.user_name);
        let ai_message = if let Some(ai_client) = &self.ai_client {
            match ai_client.generate_response_without_history(&ai_prompt).await {
                Ok(message) => message,
                Err(e) => {
                    eprintln!("Failed to generate AI message: {}", e);
                    "joins coin game!".to_string() // Fallback message
                }
            }
        } else {
            "hjoins coin game!".to_string() // Fallback message if AI client is not available
        };

        // Generate new prompt
        let new_prompt = format!("{} {}! Cost is {} pawmarks!", redemption.user_name, ai_message, new_price);

        // Update the reward cost and prompt
        if let Err(e) = self.api_client.update_custom_reward(
            &redemption.reward_id,
            &redemption.reward_title,
            new_price,
            true,
            settings.cooldown,
            &new_prompt
        ).await {
            eprintln!("Failed to update reward: {}", e);
            return RedemptionResult {
                success: false,
                message: Some("Failed to update reward".to_string()),
                queue_number: redemption.queue_number,
            };
        }

        // Send a message to the chat
        let chat_message = format!(
            "{} {}! Uploaded {} pawmarks. Cost is now {} pawmarks! Who's next?",
            redemption.user_name, ai_message, current_price, new_price
        );
        if let Err(e) = irc_client.say(channel.to_string(), chat_message).await {
            eprintln!("Failed to send message to chat: {}", e);
        }

        // Update the state
        state.current_price = new_price;
        state.last_redemption = Some(redemption.clone());

        RedemptionResult {
            success: true,
            message: Some(format!("Coin game! {}. Cost is now {} points", ai_message, new_price)),
            queue_number: redemption.queue_number,
        }
    }

    pub async fn reset_coin_game(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut state = self.coin_game_state.write().await;
        if let Some(last_redeemer) = state.last_redemption.take() {
            println!("Resetting coin game state. Last redeemer: {}", last_redeemer.user_name);

            // Refund the last redeemer
            if let Err(e) = self.api_client.refund_channel_points(&last_redeemer.reward_id, &last_redeemer.id).await {
                eprintln!("Failed to refund last coin game redeemer: {}", e);
            }
        }

        // Reset the cost to the initial value
        state.current_price = 20;

        // Update the reward cost on Twitch
        if let Some(reward_id) = self.get_coin_game_reward_id().await {
            let handlers = self.handlers_by_id.read().await;
            if let Some(settings) = handlers.get(&reward_id) {
                let new_prompt = "Enter the coin game! The price starts at 20 pawmarks!";
                self.api_client.update_custom_reward(
                    &reward_id,
                    "coin game",
                    state.current_price,
                    true,
                    settings.cooldown,
                    new_prompt  // Add the new prompt here
                ).await?;
            }
        }

        Ok(())
    }

    async fn get_coin_game_reward_id(&self) -> Option<String> {
        let handlers_by_name = self.handlers_by_name.read().await;
        handlers_by_name.get("coin game").map(|settings| settings.reward_id.clone())
    }

    pub(crate) async fn execute_action(&self, redemption: &Redemption, config: &RedemptionActionConfig, irc_client: &Arc<TwitchIRCClientType>, channel: &str) -> RedemptionResult {
        match &config.action {
            RedemptionActionType::AIResponse => {
                if let Some(ai_client) = &self.ai_client {
                    self.handle_ai_response(redemption, ai_client).await
                } else {
                    RedemptionResult {
                        success: false,
                        message: Some("AI client not initialized".to_string()),
                        queue_number: redemption.queue_number,
                    }
                }
            },
            RedemptionActionType::AIResponseWithHistory => {
                self.handle_ai_response_with_history(redemption).await
            },
            RedemptionActionType::AIResponseWithoutHistory => {
                self.handle_ai_response_without_history(redemption).await
            },
            RedemptionActionType::OSCMessage => {
                if let Some(osc_client) = &self.osc_client {
                    actions::osc_message::handle_osc_message(redemption, osc_client)
                } else {
                    RedemptionResult {
                        success: false,
                        message: Some("OSC client not initialized".to_string()),
                        queue_number: redemption.queue_number,
                    }
                }
            },
            _ => {
                self.action_manager.execute_action(
                    &redemption.reward_title,
                    redemption,
                    &self.api_client,
                    irc_client,
                    channel,
                    self.ai_client.as_deref(),
                    self.osc_client.as_deref(),
                    self
                ).await
            }
        }
    }

    pub async fn update_or_add_reward(&self, reward: ChannelPointReward) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut handlers_by_id = self.handlers_by_id.write().await;
        let mut handlers_by_name = self.handlers_by_name.write().await;
        let existing_settings = handlers_by_id.get(&reward.id);
        let cooldown = existing_settings.map(|s| s.cooldown).unwrap_or(0); // Use existing cooldown or default to 0
        let prompt = existing_settings.map(|s| s.prompt.clone()).unwrap_or_else(|| "Enter a prompt here".to_string()); // Use existing prompt or default

        let settings = RedemptionSettings {
            reward_id: reward.id.clone(),
            title: reward.title.clone(),
            cost: reward.cost,
            action_config: self.get_action_config_for_reward(&reward.title).await,
            active: reward.is_enabled,
            cooldown,
            prompt,
            active_games: vec![],
            offline_chat_redeem: false, // Add this line
        };
        handlers_by_id.insert(reward.id.clone(), settings.clone());
        handlers_by_name.insert(reward.title.clone(), settings);
        Ok(())
    }

    pub async fn remove_reward(&self, reward_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut handlers_by_id = self.handlers_by_id.write().await;
        let mut handlers_by_name = self.handlers_by_name.write().await;
        if let Some(settings) = handlers_by_id.remove(reward_id) {
            handlers_by_name.remove(&settings.title);
        }
        Ok(())
    }

    async fn get_action_config_for_reward(&self, title: &str) -> RedemptionActionConfig {
        let configs = self.reward_configs.read().await;
        configs.get(title).cloned().unwrap_or_else(|| RedemptionActionConfig {
            action: RedemptionActionType::Custom(title.to_string()),
            queued: false,
            announce_in_chat: false,
            requires_manual_completion: false,
        })
    }

    pub async fn register_reward_config(&self, title: String, config: RedemptionActionConfig) {
        let mut configs = self.reward_configs.write().await;
        configs.insert(title, config);
    }

    pub async fn update_twitch_redeems(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting to update Twitch redeems based on local settings...");

        let handlers_by_id = self.handlers_by_id.read().await;
        let status = self.stream_status.read().await;

        for (reward_id, settings) in handlers_by_id.iter() {
            println!("Checking redeem '{}' (ID: {})", settings.title, reward_id);
            match self.api_client.get_custom_reward(reward_id).await {
                Ok(current_state) => {
                    let should_be_active = self.should_be_active(settings, &status);
                    println!("Current state for '{}': is_enabled = {}, should_be_active = {}",
                             settings.title, current_state.is_enabled, should_be_active);

                    if current_state.is_enabled != should_be_active {
                        println!("Updating redeem '{}' to active = {}", settings.title, should_be_active);
                        match self.api_client.update_custom_reward(
                            reward_id,
                            &settings.title,
                            settings.cost,
                            should_be_active,
                            settings.cooldown,
                            &settings.prompt
                        ).await {
                            Ok(_) => println!("Successfully updated Twitch redeem '{}'", settings.title),
                            Err(e) => eprintln!("Failed to update Twitch redeem '{}': {}", settings.title, e),
                        }
                    } else {
                        println!("No update needed for redeem '{}'", settings.title);
                    }
                },
                Err(e) => eprintln!("Failed to get current state for redeem '{}': {}", settings.title, e),
            }
        }

        println!("Finished updating Twitch redeems.");
        Ok(())
    }

    async fn register_handler(&self, setting: RedemptionSettings) {
        let mut handlers_by_id = self.handlers_by_id.write().await;
        let mut handlers_by_name = self.handlers_by_name.write().await;
        handlers_by_id.insert(setting.reward_id.clone(), setting.clone());
        handlers_by_name.insert(setting.title.clone(), setting);
    }

    pub(crate) async fn save_settings(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let handlers_by_id = self.handlers_by_id.read().await;
        let settings: Vec<RedemptionSettings> = handlers_by_id.values().cloned().collect();

        let json = serde_json::to_string_pretty(&settings)?;
        tokio::fs::write(&self.settings_file, json).await?;
        println!("Redemption settings saved to {}", self.settings_file);

        Ok(())
    }

    async fn load_settings(&self) -> Result<Vec<RedemptionSettings>, Box<dyn std::error::Error + Send + Sync>> {
        match tokio::fs::read_to_string(&self.settings_file).await {
            Ok(contents) => {
                let settings: Vec<RedemptionSettings> = serde_json::from_str(&contents)?;
                Ok(settings)
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                println!("Redemption settings file not found. Creating with default settings.");
                let default_settings = crate::twitch::redeems::defaults::get_default_redeems();
                let json = serde_json::to_string_pretty(&default_settings)?;
                tokio::fs::write(&self.settings_file, json).await?;
                Ok(default_settings)
            },
            Err(e) => Err(e.into()),
        }
    }

    pub async fn update_stream_status(&self, is_live: bool, game: String) {
        let mut status = self.stream_status.write().await;
        *status = StreamStatus::new(is_live, game.clone());
        drop(status);  // Release the write lock

        self.update_active_redeems(is_live, &game).await;
    }

    async fn update_active_redeems(&self, is_live: bool, current_game: &str) {
        let mut handlers_by_id = self.handlers_by_id.write().await;
        for (_, settings) in handlers_by_id.iter_mut() {
            if is_live {
                settings.active = settings.active_games.is_empty() || settings.active_games.contains(&current_game.to_string());
            } else {
                settings.active = settings.offline_chat_redeem;
            }
        }
        drop(handlers_by_id);  // Release the write lock

        self.update_twitch_redeems().await.unwrap_or_else(|e| eprintln!("Failed to update Twitch redeems: {}", e));
    }

    fn should_be_active(&self, settings: &RedemptionSettings, status: &StreamStatus) -> bool {
        if !status.is_live {
            return settings.offline_chat_redeem;
        }
        settings.active_games.is_empty() || settings.active_games.contains(&status.current_game)
    }

    pub async fn add_redeem_at_runtime(
        &self,
        title: String,
        cost: u32,
        action_config: RedemptionActionConfig,
        custom_action: Option<Box<dyn RedeemAction>>,
        cooldown: u32,
        prompt: String,
        active_games: Vec<String>,
        offline_chat_redeem: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Create the reward on Twitch
        let new_reward = self.api_client.create_custom_reward(
            &title,
            cost,
            action_config.queued,
            cooldown,
            &prompt
        ).await?;

        // Create the RedemptionSettings
        let new_setting = RedemptionSettings {
            reward_id: new_reward.id.clone(),
            title: title.clone(),
            cost,
            action_config: action_config.clone(),
            active: true,
            cooldown,
            prompt,
            active_games,
            offline_chat_redeem,
        };

        // Add to handlers
        {
            let mut handlers_by_id = self.handlers_by_id.write().await;
            let mut handlers_by_name = self.handlers_by_name.write().await;
            handlers_by_id.insert(new_reward.id.clone(), new_setting.clone());
            handlers_by_name.insert(title.clone(), new_setting.clone());
        }

        // Register the action with DynamicActionManager
        match &action_config.action {
            RedemptionActionType::AIResponse => {
                self.action_manager.register_action(&title, Box::new(AIResponseAction)).await;
            }
            RedemptionActionType::OSCMessage => {
                self.action_manager.register_action(&title, Box::new(OSCMessageAction)).await;
            }
            RedemptionActionType::Custom(_) => {
                if let Some(custom_action) = custom_action {
                    self.action_manager.register_action(&title, custom_action).await;
                } else {
                    // For custom actions, if no custom action is provided, use a default one
                    self.action_manager.register_action(&title, Box::new(DefaultCustomAction)).await;
                }
            }
            _ => {
                return Err("Unsupported action type".into());
            }
        }

        // Save the updated settings to file
        self.save_settings().await?;

        Ok(())
    }

    pub async fn announce_redeems(&self, client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>, channel: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        client.say(channel.to_string(), "Bot is now active and managing redemptions!".to_string()).await?;
        let handlers = self.handlers_by_id.read().await;
        let stream_status = self.stream_status.read().await;

        for (_, settings) in handlers.iter() {
            let should_announce = if stream_status.is_live {
                settings.active
            } else {
                settings.offline_chat_redeem
            };

            if should_announce && settings.action_config.announce_in_chat {
                let message = format!("Channel point reward available: {} (Cost: {} points)", settings.title, settings.cost);
                client.say(channel.to_string(), message).await?;
            }
        }
        Ok(())
    }

    pub async fn cancel_redemption(&self, redemption_id: &str) -> Result<(), String> {
        let mut queue = self.queue.lock().await;
        let mut processed = self.processed_redemptions.lock().await;
        processed.remove(redemption_id);
        if let Some(pos) = queue.iter().position(|r| r.as_ref().map_or(false, |r| r.id == redemption_id)) {
            let redemption = queue.remove(pos).flatten();
            drop(queue);

            if let Some(redemption) = redemption {
                self.update_redemption_status(&redemption, RedemptionStatus::Canceled).await;
                Ok(())
            } else {
                Err("Redemption was None".to_string())
            }
        } else {
            Err("Redemption not found".to_string())
        }
    }

    pub async fn complete_redemption(&self, redemption_id: &str) -> Result<(), String> {
        let mut queue = self.queue.lock().await;
        if let Some(pos) = queue.iter().position(|r| r.as_ref().map_or(false, |r| r.id == redemption_id)) {
            let redemption = queue.remove(pos).flatten();
            drop(queue);

            if let Some(redemption) = redemption {
                self.update_redemption_status(&redemption, RedemptionStatus::Fulfilled).await;
                Ok(())
            } else {
                Err("Redemption was None".to_string())
            }
        } else {
            Err("Redemption not found".to_string())
        }
    }

    pub(crate) async fn update_redemption_status(&self, redemption: &Redemption, status: RedemptionStatus) {
        let status_str = match status {
            RedemptionStatus::Unfulfilled => "UNFULFILLED",
            RedemptionStatus::Fulfilled => "FULFILLED",
            RedemptionStatus::Canceled => "CANCELED",
        };

        if let Err(e) = channel_points::update_redemption_status(
            &self.api_client,
            &redemption.broadcaster_id,
            &redemption.reward_id,
            &redemption.id,
            status_str,
        ).await {
            eprintln!("Failed to update redemption status: {}", e);
        }
    }

    async fn is_moderator(&self, user_id: &str) -> bool {
        // Implement moderator check logic here
        self.api_client.is_user_moderator(user_id).await.unwrap_or(false)
    }

    pub async fn get_redemption_result(&self, redemption_id: &str) -> Result<RedemptionResult, Box<dyn std::error::Error + Send + Sync>> {
        let results = self.redemption_results.read().await;
        match results.get(redemption_id) {
            Some(result) => Ok(result.clone()),
            None => Err("Redemption result not found".into()),
        }
    }

    pub async fn get_handler_count(&self) -> usize {
        self.handlers_by_id.read().await.len()
    }

    pub async fn register_custom_action(&self, name: String, action: Box<dyn RedeemAction>) {
        self.action_manager.register_action(&name, action).await;
    }

    pub fn register_ai_response_redeem(&mut self, redeem_id: String, config: AIResponseConfig) {
        self.ai_response_manager.add_config(redeem_id, config);
    }

    async fn handle_ai_response_with_history(&self, redemption: &Redemption) -> RedemptionResult {
        if let Some(ai_client) = &self.ai_client {
            let user_input = redemption.user_input.clone().unwrap_or_default();
            match ai_client.generate_response_with_history(&user_input).await {
                Ok(response) => RedemptionResult {
                    success: true,
                    message: Some(response),
                    queue_number: redemption.queue_number,
                },
                Err(e) => RedemptionResult {
                    success: false,
                    message: Some(format!("Failed to generate AI response: {}", e)),
                    queue_number: redemption.queue_number,
                },
            }
        } else {
            RedemptionResult {
                success: false,
                message: Some("AI client not initialized".to_string()),
                queue_number: redemption.queue_number,
            }
        }
    }

    async fn handle_ai_response_without_history(&self, redemption: &Redemption) -> RedemptionResult {
        if let Some(ai_client) = &self.ai_client {
            let user_input = redemption.user_input.clone().unwrap_or_default();
            match ai_client.generate_response_without_history(&user_input).await {
                Ok(response) => RedemptionResult {
                    success: true,
                    message: Some(response),
                    queue_number: redemption.queue_number,
                },
                Err(e) => RedemptionResult {
                    success: false,
                    message: Some(format!("Failed to generate AI response: {}", e)),
                    queue_number: redemption.queue_number,
                },
            }
        } else {
            RedemptionResult {
                success: false,
                message: Some("AI client not initialized".to_string()),
                queue_number: redemption.queue_number,
            }
        }
    }
}

struct DefaultCustomAction;

#[async_trait]
impl RedeemAction for DefaultCustomAction {
    async fn execute(&self, redemption: &Redemption, api_client: &TwitchAPIClient, irc_client: &Arc<TwitchIRCClientType>, channel: &str, ai_client: Option<&AIClient>, osc_client: Option<&VRChatOSC>, redeem_manager: &RedeemManager) -> RedemptionResult {
        RedemptionResult {
            success: true,
            message: Some("Custom redemption executed".to_string()),
            queue_number: redemption.queue_number,
        }
    }
}

impl RedeemManager {
    pub async fn handle_ai_response(&self, redemption: &Redemption, ai_client: &AIClient) -> RedemptionResult {
        println!("title: {}", &redemption.reward_title);
        let config = match self.ai_response_manager.get_config(&redemption.reward_title) {
            Some(config) => config,
            None => return RedemptionResult {
                success: false,
                message: Some("AI response configuration not found".to_string()),
                queue_number: redemption.queue_number,
            },
        };

        let user_input = redemption.user_input.clone().unwrap_or_default();
        let full_prompt = format!("{}\n{}", config.prompt, user_input);

        let result = match &config.provider {
            AIProvider::OpenAI => {
                ai_client.generate_openai_response(&config.model, &full_prompt, config.max_tokens, config.temperature).await
            }
            AIProvider::Anthropic => {
                ai_client.generate_anthropic_response(&config.model, &full_prompt, config.max_tokens, config.temperature).await
            }
            AIProvider::Local => {
                ai_client.generate_local_response(&config.model, &full_prompt, config.max_tokens, config.temperature).await
            }
        };

        match result {
            Ok(response) => RedemptionResult {
                success: true,
                message: Some(response),
                queue_number: redemption.queue_number,
            },
            Err(e) => RedemptionResult {
                success: false,
                message: Some(format!("Failed to generate AI response: {}", e)),
                queue_number: redemption.queue_number,
            },
        }
    }

    pub async fn handle_update_text(&self, redemption: &Redemption) -> RedemptionResult {
        // Implement text update logic here
        // This is a placeholder implementation
        RedemptionResult {
            success: true,
            message: Some("Text updated successfully".to_string()),
            queue_number: redemption.queue_number,
        }
    }

    pub async fn handle_refund(&self, redemption: &Redemption) -> RedemptionResult {
        match channel_points::refund_channel_points(
            &self.api_client,
            &redemption.broadcaster_id,
            &redemption.reward_id,
            &redemption.id,
        ).await {
            Ok(_) => RedemptionResult {
                success: true,
                message: Some("Redemption refunded successfully".to_string()),
                queue_number: redemption.queue_number,
            },
            Err(e) => RedemptionResult {
                success: false,
                message: Some(format!("Refund error: {}", e)),
                queue_number: redemption.queue_number,
            },
        }
    }

    async fn enqueue_redemption(&self, redemption: &mut Redemption) {
        if redemption.queued {
            let mut queue = self.queue.lock().await;
            let mut next_number = self.next_queue_number.lock().await;
            redemption.queue_number = Some(*next_number);
            *next_number += 1;
            queue.push_back(Some(redemption.clone()));
        }
    }

    async fn process_result(&self, redemption: &Redemption, result: &RedemptionResult) {
        let status = if result.success {
            RedemptionStatus::Fulfilled
        } else {
            RedemptionStatus::Canceled
        };
        self.update_redemption_status(redemption, status).await;
    }

    fn get_special_redeems(&self) -> HashSet<String> {
        // Define your special redeems here
        vec![
            "AI Response".to_string(),
            "OSC Message".to_string(),
            "Update Text".to_string(),
            "coin game".to_string(),
            // Add more special redeems as needed
        ].into_iter().collect()
    }

    fn get_special_redeem_config(&self, title: &str) -> RedemptionActionConfig {
        match title {
            "AI Response" => RedemptionActionConfig {
                action: RedemptionActionType::AIResponse,
                queued: true,
                announce_in_chat: true,
                requires_manual_completion: false,
            },
            "OSC Message" => RedemptionActionConfig {
                action: RedemptionActionType::OSCMessage,
                queued: false,
                announce_in_chat: false,
                requires_manual_completion: false,
            },
            "Update Text" => RedemptionActionConfig {
                action: RedemptionActionType::UpdateText,
                queued: false,
                announce_in_chat: true,
                requires_manual_completion: false,
            },
            "coin game" => RedemptionActionConfig {
                action: RedemptionActionType::Custom("coin game".to_string()),
                queued: false,
                announce_in_chat: true,
                requires_manual_completion: false,
            },
            // Add more special redeems as needed
            _ => RedemptionActionConfig {
                action: RedemptionActionType::Custom(title.to_string()),
                queued: false,
                announce_in_chat: false,
                requires_manual_completion: false,
            },
        }
    }
}

impl RedeemManager {

}
