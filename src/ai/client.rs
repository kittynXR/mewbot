use std::sync::Arc;
use tokio::sync::RwLock;
use super::models::{AIProvider, AIError};
use super::openai::OpenAIProvider;
use super::anthropic::AnthropicProvider;

pub struct AIClient {
    openai_provider: Option<OpenAIProvider>,
    anthropic_provider: Option<AnthropicProvider>,
    chat_history: Arc<RwLock<String>>,
}

impl AIClient {
    pub fn new(openai_api_key: Option<String>, anthropic_api_key: Option<String>) -> Self {
        Self {
            openai_provider: openai_api_key.map(OpenAIProvider::new),
            anthropic_provider: anthropic_api_key.map(AnthropicProvider::new),
            chat_history: Arc::new(RwLock::new(String::new())),
        }
    }

    pub async fn update_chat_history(&self, message: &str) {
        let mut history = self.chat_history.write().await;
        history.push_str(message);
        history.push_str("\n");

        // Increase the token limit to maintain a larger context window
        // This is an approximation; adjust based on your needs and the model's capabilities
        while Self::count_tokens(&history) > 100000 {
            if let Some(newline_pos) = history.find('\n') {
                history.drain(..=newline_pos);
            } else {
                break;
            }
        }
    }

    fn count_tokens(text: &str) -> usize {
        // Simple approximation: split on whitespace and count
        text.split_whitespace().count()
    }

    pub async fn generate_response_with_history(&self, prompt: &str) -> Result<String, AIError> {
        let history = self.chat_history.read().await;
        let full_prompt = format!("{}\n\n{}", *history, prompt);
        self.generate_response(&full_prompt).await
    }

    pub async fn generate_response_without_history(&self, prompt: &str) -> Result<String, AIError> {
        self.generate_response(prompt).await
    }

    async fn generate_response(&self, prompt: &str) -> Result<String, AIError> {
        // Default to OpenAI if available, otherwise use Anthropic
        if let Some(provider) = &self.openai_provider {
            provider.generate_response(prompt).await
        } else if let Some(provider) = &self.anthropic_provider {
            provider.generate_response(prompt).await
        } else {
            Err(AIError::APIError("No AI provider available".to_string()))
        }
    }

    pub async fn generate_openai_response(&self, model: &str, prompt: &str, max_tokens: usize, temperature: f32) -> Result<String, AIError> {
        if let Some(provider) = &self.openai_provider {
            // For now, we'll ignore the model, max_tokens, and temperature parameters
            provider.generate_response(prompt).await
        } else {
            Err(AIError::APIError("OpenAI provider not available".to_string()))
        }
    }

    pub async fn generate_anthropic_response(&self, model: &str, prompt: &str, max_tokens: usize, temperature: f32) -> Result<String, AIError> {
        if let Some(provider) = &self.anthropic_provider {
            // For now, we'll ignore the model, max_tokens, and temperature parameters
            provider.generate_response(prompt).await
        } else {
            Err(AIError::APIError("Anthropic provider not available".to_string()))
        }
    }

    pub async fn generate_local_response(&self, model: &str, prompt: &str, max_tokens: usize, temperature: f32) -> Result<String, AIError> {
        // Implement local model response generation
        // For now, we'll return an error as it's not implemented
        Err(AIError::APIError("Local AI provider not implemented".to_string()))
    }
}