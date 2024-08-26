use async_trait::async_trait;
use std::sync::Arc;
use crate::ai::AIClient;
use crate::twitch::redeems::models::{Redemption, RedemptionResult, RedeemHandler};

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
        let prompt = "You are an entertaining chatbot.  Your owner is the cute and funny catgirl named kittyn.\
                                Answer questions humorously and in a playful manner.   \
                                we sometimes stream with the cookiesquad, foxly, only nekos, and only cans stream groups \
                                be friendly and love chat".to_string();
        match self.ai_client.generate_response_without_history(&prompt).await {
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
}