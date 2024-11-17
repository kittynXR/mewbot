use std::any::Any;
use async_trait::async_trait;
use std::sync::Arc;
use log::{error, info};
use crate::ai::AIClient;
use crate::twitch::models::{Redemption, RedemptionResult, RedeemHandler};
use super::utils::split_response;

pub struct AskAIAction {
    ai_client: Arc<AIClient>,
}

impl AskAIAction {
    pub fn new(ai_client: Arc<AIClient>) -> Self {
        Self { ai_client }
    }
}

#[async_trait]
impl RedeemHandler for AskAIAction {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        let base_prompt = "You are an entertaining chatbot. Your owner is the cute and funny catgirl named kittyn. \
                           Answer questions humorously and in a playful manner. \
                           We live on Twitch in a cozy but high-tech corner of the metaverse. \
                           Be friendly and love the chat who asks you these questions. \
                           Treat chat like they were your own children.";

        let user_input = redemption.user_input.as_deref().unwrap_or("").trim();

        if user_input.is_empty() {
            return RedemptionResult {
                success: false,
                message: Some("Please provide a question or topic for the AI to respond to.".to_string()),
            };
        }

        let full_prompt = format!("{}User's question: {}", base_prompt, user_input);

        match self.ai_client.generate_response_without_history(&full_prompt).await {
            Ok(response) => RedemptionResult {
                success: true,
                message: Some(response),
            },
            Err(e) => RedemptionResult {
                success: false,
                message: Some(format!("Failed to generate AI response: {}", e)),
            },
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct SeriousAIAction {
    ai_client: Arc<AIClient>,
}

impl SeriousAIAction {
    pub fn new(ai_client: Arc<AIClient>) -> Self {
        Self { ai_client }
    }
}

#[async_trait]
impl RedeemHandler for SeriousAIAction {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        let base_prompt = "You are a knowledgeable AI assistant. Provide a concise, accurate, and informative answer \
                           to the following question. Your response should be clear and fit within a single chat message. ";

        let user_input = redemption.user_input.as_deref().unwrap_or("").trim();

        if user_input.is_empty() {
            return RedemptionResult {
                success: false,
                message: Some("Please provide a question for the AI to answer.".to_string()),
            };
        }

        let full_prompt = format!("{}User's question: {}", base_prompt, user_input);

        match self.ai_client.generate_response_without_history(&full_prompt).await {
            Ok(response) => RedemptionResult {
                success: true,
                message: Some(response),
            },
            Err(e) => RedemptionResult {
                success: false,
                message: Some(format!("Failed to generate AI response: {}", e)),
            },
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct GrokAIAction {
    ai_client: Arc<AIClient>,
}

impl GrokAIAction {
    pub fn new(ai_client: Arc<AIClient>) -> Self {
        Self { ai_client }
    }
}

#[async_trait]
impl RedeemHandler for GrokAIAction {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        let user_input = match &redemption.user_input {
            Some(input) if !input.trim().is_empty() => input.trim(),
            _ => return RedemptionResult {
                success: false,
                message: Some("Please provide a question or topic for Grok to analyze.".to_string()),
            }
        };

        info!("Processing Grok AI request from {}: {}", redemption.user_name, user_input);

        // Only try the XAI provider for Grok
        match self.ai_client.generate_grok_response(user_input).await {
            Ok(response) => {
                let (first_part, remainder) = split_response(response);

                if let Some(remainder) = remainder {
                    info!("Storing remainder for user {}", redemption.user_id);
                    self.ai_client.store_remainder(redemption.user_id.clone(), remainder).await;

                    RedemptionResult {
                        success: true,
                        message: Some(format!("{} (Use !continue to see more)", first_part)),
                    }
                } else {
                    RedemptionResult {
                        success: true,
                        message: Some(first_part),
                    }
                }
            },
            Err(e) => {
                error!("Failed to generate Grok response: {}", e);
                RedemptionResult {
                    success: false,
                    message: Some(format!("Failed to analyze your request. Please try again later. Error: {}", e)),
                }
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}