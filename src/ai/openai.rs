// openai.rs
use async_trait::async_trait;
use super::models::{AIProvider, AIError};
use reqwest::Client;
use serde_json::json;

pub struct OpenAIProvider {
    api_key: String,
    client: Client,
}

impl OpenAIProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    async fn generate_response_with_model(&self, prompt: &str, model: &str, tokens: i32) -> Result<String, AIError> {
        let response = self.client.post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({
                "model": model,
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": tokens,
            }))
            .send()
            .await
            .map_err(|e| AIError::NetworkError(e.to_string()))?;

        let response_json: serde_json::Value = response.json()
            .await
            .map_err(|e| AIError::InvalidResponse(e.to_string()))?;

        response_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AIError::InvalidResponse("No content in response".to_string()))
    }

    pub async fn generate_web_search_response(
        &self,
        prompt: &str,
    ) -> Result<String, AIError> {

        self.generate_response_with_model(&prompt, "gpt-4", 250).await
    }
}

#[async_trait]
impl AIProvider for OpenAIProvider {
    async fn generate_response(&self, prompt: &str) -> Result<String, AIError> {
        self.generate_response_with_model(prompt, "gpt-4o-mini", 100).await
    }

    async fn generate_response_without_history(&self, prompt: &str) -> Result<String, AIError> {
        self.generate_response_with_model(prompt, "gpt-4o", 250).await
    }
}