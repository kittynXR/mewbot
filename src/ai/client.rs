use super::models::{AIProvider, AIError};
use std::sync::Arc;

pub struct AIClient {
    provider: Arc<dyn AIProvider>,
}

impl AIClient {
    pub fn new(provider: Arc<dyn AIProvider>) -> Self {
        Self { provider }
    }

    pub async fn generate_response(&self, prompt: &str) -> Result<String, AIError> {
        self.provider.generate_response(prompt).await
    }
}