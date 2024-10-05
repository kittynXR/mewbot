use std::any::Any;
use async_trait::async_trait;
use std::sync::Arc;
use crate::ai::AIClient;
use crate::twitch::models::{Redemption, RedemptionResult, RedeemHandler};

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