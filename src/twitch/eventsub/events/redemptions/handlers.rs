// src/twitch/eventsub/events/redemptions/handlers.rs

use super::models::{Redemption, RedemptionActionType, RedemptionActionConfig, RedemptionResult, RedemptionStatus, RedemptionSettings};
use crate::ai::AIClient;
use crate::osc::VRChatOSC;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::api::requests::channel_points;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::fs;
use std::path::Path;
use std::collections::HashSet;
use serde_json;


pub struct RedemptionManager {
    handlers: HashMap<String, RedemptionSettings>,
    ai_client: Option<Arc<AIClient>>,
    osc_client: Option<Arc<VRChatOSC>>,
    api_client: Arc<TwitchAPIClient>,
    queue: Mutex<VecDeque<Option<Redemption>>>,
    next_queue_number: Mutex<usize>,
    settings_file: String,
}

impl RedemptionManager {
    pub fn new(
        ai_client: Option<Arc<AIClient>>,
        osc_client: Option<Arc<VRChatOSC>>,
        api_client: Arc<TwitchAPIClient>,
    ) -> Self {
        Self {
            handlers: HashMap::new(),
            ai_client,
            osc_client,
            api_client,
            queue: Mutex::new(VecDeque::new()),
            next_queue_number: Mutex::new(1),
            settings_file: "redemption_settings.json".to_string(),
        }
    }

    pub async fn update_from_twitch(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let rewards = self.api_client.get_channel_point_rewards().await?;
        let mut updated = false;
        let special_redeems = self.get_special_redeems();
        let existing_reward_ids: HashSet<String> = self.handlers.keys().cloned().collect();

        // First, gather all the updates we need to make
        let mut updates: Vec<(String, RedemptionSettings)> = Vec::new();

        for reward in rewards {
            if !existing_reward_ids.contains(&reward.id) {
                // New reward
                let action_config = if special_redeems.contains(&reward.title) {
                    self.get_special_redeem_config(&reward.title)
                } else {
                    RedemptionActionConfig {
                        action: RedemptionActionType::Custom(reward.title.clone()),
                        queued: reward.is_user_input_required,
                        announce_in_chat: false,
                        requires_manual_completion: false,
                    }
                };

                let new_settings = RedemptionSettings {
                    reward_id: reward.id.clone(),
                    title: reward.title.clone(),
                    cost: reward.cost,
                    action_config,
                    active: true, // New rewards are active by default
                };
                updates.push((reward.id.clone(), new_settings));
                updated = true;
            } else {
                // Update existing handler with latest Twitch data
                if let Some(settings) = self.handlers.get(&reward.id) {
                    let mut new_settings = settings.clone();
                    new_settings.title = reward.title.clone();
                    new_settings.cost = reward.cost;
                    new_settings.action_config.queued = reward.is_user_input_required;

                    if special_redeems.contains(&new_settings.title) {
                        // Update special redeems
                        new_settings.action_config = self.get_special_redeem_config(&new_settings.title);
                        updated = true;
                    }

                    updates.push((reward.id.clone(), new_settings));
                }
            }
        }

        // Now apply all the updates
        for (reward_id, new_settings) in updates {
            self.handlers.insert(reward_id, new_settings);
        }

        if updated {
            self.save_settings()?;
        }

        Ok(())
    }

    pub async fn update_twitch_redeems(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Starting to update Twitch redeems based on local settings...");
        for (reward_id, settings) in &self.handlers {
            println!("Checking redeem '{}' (ID: {})", settings.title, reward_id);
            match self.api_client.get_custom_reward(reward_id).await {
                Ok(current_state) => {
                    let is_enabled = current_state["data"][0]["is_enabled"].as_bool().unwrap_or(false);
                    println!("Current state for '{}': is_enabled = {}, local active = {}",
                             settings.title, is_enabled, settings.active);

                    if is_enabled != settings.active {
                        println!("Updating redeem '{}' to active = {}", settings.title, settings.active);
                        match self.api_client.update_custom_reward(
                            reward_id,
                            &settings.title,
                            settings.cost,
                            settings.active,
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

    pub fn register_handler(&mut self, settings: RedemptionSettings) {
        self.handlers.insert(settings.reward_id.clone(), settings);
    }

    pub fn save_settings(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let settings: Vec<RedemptionSettings> = self.handlers.values().cloned().collect();

        let json = serde_json::to_string_pretty(&settings)?;
        fs::write(&self.settings_file, json)?;
        println!("Redemption settings saved to {}", self.settings_file);

        Ok(())
    }

    pub fn load_settings(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let path = Path::new(&self.settings_file);
        if !path.exists() {
            println!("Redemption settings file not found. Creating a new one.");
            self.save_settings()?;
            return Ok(());
        }

        let contents = fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            println!("Redemption settings file is empty. Using default settings.");
            return Ok(());
        }

        let settings: Vec<RedemptionSettings> = serde_json::from_str(&contents)?;

        for setting in settings {
            self.register_handler(setting);
        }

        Ok(())
    }

    pub async fn handle_redemption(&self, mut redemption: Redemption) -> RedemptionResult {
        if let Some(settings) = self.handlers.get(&redemption.reward_id) {
            if !settings.active {
                return RedemptionResult {
                    success: false,
                    message: Some("This redemption is currently inactive".to_string()),
                    queue_number: None,
                };
            }

            redemption.queued = settings.action_config.queued;
            redemption.announce_in_chat = settings.action_config.announce_in_chat;

            self.enqueue_redemption(&mut redemption).await;

            let result = self.execute_action(&redemption, &settings.action_config).await;

            if !settings.action_config.requires_manual_completion {
                self.process_result(&redemption, &result).await;
            }

            result
        } else {
            RedemptionResult {
                success: false,
                message: Some("No handler registered for this reward".to_string()),
                queue_number: None,
            }
        }
    }

    fn get_special_redeems(&self) -> HashSet<String> {
        // Define your special redeems here
        vec![
            "AI Response".to_string(),
            "OSC Message".to_string(),
            "Update Text".to_string(),
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
            // Add more special redeems as needed
            _ => RedemptionActionConfig {
                action: RedemptionActionType::Custom(title.to_string()),
                queued: false,
                announce_in_chat: false,
                requires_manual_completion: false,
            },
        }
    }

    // pub fn register_handler(&mut self, reward_id: String, config: RedemptionActionConfig) {
    //     self.handlers.insert(reward_id, config);
    // }

    async fn process_result(&self, redemption: &Redemption, result: &RedemptionResult) {
        let status = if result.success {
            RedemptionStatus::Fulfilled
        } else {
            RedemptionStatus::Canceled
        };
        self.update_redemption_status(redemption, status).await;
    }

    async fn execute_action(&self, redemption: &Redemption, config: &RedemptionActionConfig) -> RedemptionResult {
        match &config.action {
            RedemptionActionType::AIResponse => self.handle_ai_response(redemption).await,
            RedemptionActionType::OSCMessage => self.handle_osc_message(redemption).await,
            RedemptionActionType::UpdateText => self.handle_update_text(redemption).await,
            RedemptionActionType::Refund => self.handle_refund(redemption).await,
            RedemptionActionType::Custom(action_name) => self.handle_custom_action(redemption, action_name).await,
        }
    }

    async fn handle_custom_action(&self, redemption: &Redemption, action_name: &str) -> RedemptionResult {
        match action_name {
            "some_custom_action" => {
                // Implement your custom action logic here
                RedemptionResult {
                    success: true,
                    message: Some("Custom action executed".to_string()),
                    queue_number: redemption.queue_number,
                }
            },
            // Add more custom actions as needed
            _ => RedemptionResult {
                success: false,
                message: Some(format!("Unknown custom action: {}", action_name)),
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

    pub async fn complete_redemption(&self, redemption_id: &str, user_id: &str) -> Result<(), String> {
        let mut queue = self.queue.lock().await;
        let position = queue.iter().position(|r| r.as_ref().map_or(false, |r| r.id == redemption_id))
            .ok_or_else(|| "Redemption not found".to_string())?;

        // Get a reference to the redemption without removing it from the queue yet
        let redemption = queue[position].as_ref().ok_or_else(|| "Redemption is None".to_string())?;

        // Check if the redemption requires manual completion
        let config = self.handlers.get(&redemption.reward_id)
            .ok_or_else(|| "No handler found for this redemption".to_string())?;

        if !config.action_config.requires_manual_completion {
            return Err("This redemption does not require manual completion".to_string());
        }

        // Check if the user is authorized
        if redemption.user_id == user_id || self.is_moderator(user_id).await {
            // Remove the redemption from the queue
            let redemption = queue.remove(position).flatten();

            // Drop the lock on the queue before updating the status
            drop(queue);

            // Update the status
            if let Some(redemption) = redemption {
                self.update_redemption_status(&redemption, RedemptionStatus::Fulfilled).await;
                Ok(())
            } else {
                Err("Redemption was None".to_string())
            }
        } else {
            Err("User not authorized to complete this redemption".to_string())
        }
    }

    async fn handle_ai_response(&self, redemption: &Redemption) -> RedemptionResult {
        if let Some(ai_client) = &self.ai_client {
            // Implement AI response logic here
            // This is a placeholder implementation
            let response = ai_client.generate_response(&redemption.user_input.clone().unwrap_or_default()).await;
            match response {
                Ok(message) => RedemptionResult {
                    success: true,
                    message: Some(message),
                    queue_number: redemption.queue_number,
                },
                Err(e) => RedemptionResult {
                    success: false,
                    message: Some(format!("AI error: {}", e)),
                    queue_number: redemption.queue_number,
                },
            }
        } else {
            RedemptionResult {
                success: false,
                message: Some("AI client not configured".to_string()),
                queue_number: redemption.queue_number,
            }
        }
    }

    async fn handle_osc_message(&self, redemption: &Redemption) -> RedemptionResult {
        if let Some(osc_client) = &self.osc_client {
            // Implement OSC message sending logic here
            // This is a placeholder implementation
            let result = osc_client.send_message("/avatar/parameters/SomeParameter", rosc::OscType::Float(1.0));
            match result {
                Ok(_) => RedemptionResult {
                    success: true,
                    message: Some("OSC message sent successfully".to_string()),
                    queue_number: redemption.queue_number,
                },
                Err(e) => RedemptionResult {
                    success: false,
                    message: Some(format!("OSC error: {}", e)),
                    queue_number: redemption.queue_number,
                },
            }
        } else {
            RedemptionResult {
                success: false,
                message: Some("OSC client not configured".to_string()),
                queue_number: redemption.queue_number,
            }
        }
    }

    async fn handle_update_text(&self, redemption: &Redemption) -> RedemptionResult {
        // Implement text update logic here
        // This is a placeholder implementation
        RedemptionResult {
            success: true,
            message: Some("Text updated successfully".to_string()),
            queue_number: redemption.queue_number,
        }
    }

    async fn handle_refund(&self, redemption: &Redemption) -> RedemptionResult {
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

    async fn update_redemption_status(&self, redemption: &Redemption, status: RedemptionStatus) {
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
}