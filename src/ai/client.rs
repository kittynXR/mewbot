use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;
use log::{debug};
use crate::ai::{SearchProvider, WebSearchClient};
use super::models::{AIProvider, AIError};
use super::openai::OpenAIProvider;
use super::anthropic::AnthropicProvider;
use super::xai::XAIProvider;

pub struct AIClient {
    openai_provider: Option<OpenAIProvider>,
    anthropic_provider: Option<AnthropicProvider>,
    xai_provider: Option<XAIProvider>,
    web_search_client: WebSearchClient,
    chat_history: Arc<RwLock<String>>,
    message_buffer: Arc<RwLock<HashMap<String, String>>>,
}

impl AIClient {
    pub fn new(
        openai_api_key: Option<String>,
        anthropic_api_key: Option<String>,
        xai_api_key: Option<String>,
        google_search_api_key: Option<String>,
        google_search_cx: Option<String>,
        bing_search_api_key: Option<String>,
    ) -> Self {
        Self {
            openai_provider: openai_api_key.map(OpenAIProvider::new),
            anthropic_provider: anthropic_api_key.map(AnthropicProvider::new),
            xai_provider: xai_api_key.map(XAIProvider::new),
            web_search_client: WebSearchClient::new(
                google_search_api_key,
                google_search_cx,
                bing_search_api_key,
            ),
            chat_history: Arc::new(RwLock::new(String::new())),
            message_buffer: Arc::new(RwLock::new(HashMap::new())),
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
        if let Some(provider) = &self.openai_provider {
            provider.generate_response_without_history(prompt).await
        } else if let Some(provider) = &self.anthropic_provider {
            // Assuming Anthropic provider also implements generate_response_without_history
            provider.generate_response_without_history(prompt).await
        } else {
            Err(AIError::APIError("No AI provider available".to_string()))
        }
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

    pub async fn store_remainder(&self, user_id: String, remainder: String) {
        let mut buffer = self.message_buffer.write().await;
        let user_id_clone = user_id.clone(); // Clone for the debug message
        buffer.insert(user_id, remainder);
        debug!("Stored remainder for user {}", user_id_clone);
    }

    pub async fn get_remainder(&self, user_id: &str) -> Option<String> {
        let mut buffer = self.message_buffer.write().await;
        let remainder = buffer.remove(user_id);
        debug!("Retrieved remainder for user {}: {:?}", user_id, remainder.is_some());
        remainder
    }

    pub async fn generate_web_search_response(&self, prompt: &str) -> Result<String, AIError> {
        if let Some(provider) = &self.openai_provider {
            // Perform web search
            let search_results = self.web_search_client
                .search(prompt, SearchProvider::Bing, 5)
                .await
                .map_err(|e| AIError::APIError(format!("Web search failed: {}", e)))?;

            let formatted_results = WebSearchClient::format_results(&search_results);

            // Create the full prompt with search results
            let full_prompt = format!(
                "Based on the following web search results:\n\n{}\n\nProvide a comprehensive but concise answer to: {}",
                formatted_results,
                prompt
            );

            // Generate response using OpenAI
            provider.generate_web_search_response(&full_prompt).await
        } else {
            Err(AIError::APIError("OpenAI provider not available for web search".to_string()))
        }
    }

    pub async fn generate_grok_response(&self, prompt: &str) -> Result<String, AIError> {
        if let Some(provider) = &self.xai_provider {
            provider.generate_grok_response(prompt).await
        } else {
            Err(AIError::APIError("XAI provider not available for Grok responses".to_string()))
        }
    }
}