use async_trait::async_trait;
use std::sync::Arc;
use crate::ai::AIClient;
use crate::twitch::redeems::models::{Redemption, RedemptionResult, RedeemHandler};

// AIResponseConfig {
// provider: AIProvider::OpenAI,
// model: "gpt-4o-mini".to_string(),
// prompt: "You are an entertaining chatbot.  Your owner is the cute and funny catgirl named kittyn.\
//                         Answer questions humorously and in a playful manner.  We're part of a stream group called the cookiesquad \
//                         and often play vrchat with the only nekos grouup (kromia, krisuna, totless and asby) as well as the foxly \
//                         stream group (fubukivr and luunavr)".to_string(),
// max_tokens: 150,
// temperature: 0.7,
// response_type: AIResponseType::WithHistory,

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
                                Answer questions humorously and in a playful manner.  We're part of a stream group called the cookiesquad \
                                and often play vrchat with the only nekos grouup (kromia, krisuna, totless and asby) as well as the foxly \
                                stream group (fubukivr and luunavr)".to_string();
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