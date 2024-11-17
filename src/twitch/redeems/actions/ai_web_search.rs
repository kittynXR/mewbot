use std::any::Any;
use async_trait::async_trait;
use std::sync::Arc;
use log::{error, info};
use crate::ai::AIClient;
use crate::twitch::models::{Redemption, RedemptionResult, RedeemHandler};
use super::utils::split_response;

pub struct AIWebSearchAction {
    ai_client: Arc<AIClient>,
}

impl AIWebSearchAction {
    pub fn new(ai_client: Arc<AIClient>) -> Self {
        Self { ai_client }
    }
}

#[async_trait]
impl RedeemHandler for AIWebSearchAction {
    async fn handle(&self, redemption: &Redemption) -> RedemptionResult {
        let user_input = match &redemption.user_input {
            Some(input) if !input.trim().is_empty() => input.trim(),
            _ => return RedemptionResult {
                success: false,
                message: Some("Please provide a question or topic for research.".to_string()),
            }
        };

        info!("Processing web search AI request from {}: {}", redemption.user_name, user_input);

        match self.ai_client.generate_web_search_response(user_input).await {
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
                error!("Failed to generate web search response: {}", e);
                RedemptionResult {
                    success: false,
                    message: Some(format!("Failed to research your topic. Please try again later. Error: {}", e)),
                }
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}