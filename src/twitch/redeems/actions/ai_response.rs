use std::collections::HashMap;
use crate::twitch::redeems::RedeemAction;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::irc::client::TwitchIRCClientType;
use crate::osc::VRChatOSC;
use std::sync::Arc;
use crate::ai::{AIClient, AIError};
use crate::twitch::redeems::models::{Redemption, RedemptionResult};
use crate::twitch::redeems::manager::RedeemManager;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};


#[derive(Clone, Serialize, Deserialize)]
pub enum AIProvider {
    OpenAI,
    Anthropic,
    Local,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum AIResponseType {
    Standard,
    WithHistory,
    WithoutHistory,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AIResponseConfig {
    pub provider: AIProvider,
    pub model: String,
    pub prompt: String,
    pub max_tokens: usize,
    pub temperature: f32,
    pub response_type: AIResponseType,
}

pub struct AIResponseManager {
    configs: HashMap<String, AIResponseConfig>,
}

impl AIResponseManager {
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
        }
    }

    pub fn add_config(&mut self, redeem_id: String, config: AIResponseConfig) {
        self.configs.insert(redeem_id, config);
    }

    pub fn remove_config(&mut self, redeem_id: &str) {
        self.configs.remove(redeem_id);
    }

    pub async fn handle_ai_response(&self, redemption: &Redemption, ai_client: &AIClient) -> RedemptionResult {
        let config = match self.configs.get(&redemption.reward_id) {
            Some(config) => config,
            None => return RedemptionResult {
                success: false,
                message: Some("AI response configuration not found".to_string()),
                queue_number: redemption.queue_number,
            },
        };

        let user_input = redemption.user_input.clone().unwrap_or_default();
        let full_prompt = format!("{}\n{}", config.prompt, user_input);

        let result = match config.response_type {
            AIResponseType::Standard | AIResponseType::WithHistory => {
                ai_client.generate_response_with_history(&full_prompt).await
            },
            AIResponseType::WithoutHistory => {
                ai_client.generate_response_without_history(&full_prompt).await
            },
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

    async fn generate_openai_response(&self, ai_client: &AIClient, config: &AIResponseConfig, prompt: &str) -> RedemptionResult {
        match ai_client.generate_openai_response(&config.model, prompt, config.max_tokens, config.temperature).await {
            Ok(response) => RedemptionResult {
                success: true,
                message: Some(response),
                queue_number: None,
            },
            Err(e) => RedemptionResult {
                success: false,
                message: Some(format!("Failed to generate OpenAI response: {}", e)),
                queue_number: None,
            },
        }
    }

    async fn generate_anthropic_response(&self, ai_client: &AIClient, config: &AIResponseConfig, prompt: &str) -> RedemptionResult {
        match ai_client.generate_anthropic_response(&config.model, prompt, config.max_tokens, config.temperature).await {
            Ok(response) => RedemptionResult {
                success: true,
                message: Some(response),
                queue_number: None,
            },
            Err(e) => RedemptionResult {
                success: false,
                message: Some(format!("Failed to generate Anthropic response: {}", e)),
                queue_number: None,
            },
        }
    }

    async fn generate_local_response(&self, ai_client: &AIClient, config: &AIResponseConfig, prompt: &str) -> RedemptionResult {
        match ai_client.generate_local_response(&config.model, prompt, config.max_tokens, config.temperature).await {
            Ok(response) => RedemptionResult {
                success: true,
                message: Some(response),
                queue_number: None,
            },
            Err(e) => RedemptionResult {
                success: false,
                message: Some(format!("Failed to generate local response: {}", e)),
                queue_number: None,
            },
        }
    }

    pub fn get_config(&self, redeem_id: &str) -> Option<&AIResponseConfig> {
        self.configs.get(redeem_id)
    }
}

pub struct AIResponseAction;
#[async_trait]
impl RedeemAction for AIResponseAction {
    async fn execute(&self, redemption: &Redemption, api_client: &TwitchAPIClient, irc_client: &Arc<TwitchIRCClientType>, channel: &str, ai_client: Option<&AIClient>, _osc_client: Option<&VRChatOSC>, redeem_manager: &RedeemManager) -> RedemptionResult {
        if let Some(ai_client) = ai_client {
            let result = redeem_manager.handle_ai_response(redemption, ai_client).await;

            if result.success {
                // Mark the redemption as complete
                if let Err(e) = api_client.update_redemption_status(&redemption.reward_id, &redemption.id, "FULFILLED").await {
                    eprintln!("Failed to mark redemption as complete: {}", e);
                }

                // Send the AI response to chat
                if let Some(message) = &result.message {
                    let chat_message = format!("@{}: {}", redemption.user_name, message);
                    if let Err(e) = irc_client.say(channel.to_string(), chat_message).await {
                        eprintln!("Failed to send message to chat: {}", e);
                    }
                }
            }

            result
        } else {
            RedemptionResult {
                success: false,
                message: Some("AI client not initialized".to_string()),
                queue_number: redemption.queue_number,
            }
        }
    }
}