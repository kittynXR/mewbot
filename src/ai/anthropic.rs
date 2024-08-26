use async_trait::async_trait;
use super::models::{AIProvider, AIError};
use reqwest::Client;
use serde_json::json;

pub struct AnthropicProvider {
    api_key: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    async fn generate_response_with_model(&self, prompt: &str, model: &str) -> Result<String, AIError> {
        let response = self.client.post("https://api.anthropic.com/v1/messages")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": model,
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": 1000
            }))
            .send()
            .await
            .map_err(|e| AIError::NetworkError(e.to_string()))?;

        let response_json: serde_json::Value = response.json()
            .await
            .map_err(|e| AIError::InvalidResponse(e.to_string()))?;

        response_json["content"][0]["text"]
            .as_str()
            .map(|s| s.trim().to_string())
            .ok_or_else(|| AIError::InvalidResponse("No content in response".to_string()))
    }
}

#[async_trait]
impl AIProvider for AnthropicProvider {
    async fn generate_response(&self, prompt: &str) -> Result<String, AIError> {
        // Use Claude 3.0 for responses with history
        self.generate_response_with_model(prompt, "claude-3-opus-20240229").await
    }

    async fn generate_response_without_history(&self, prompt: &str) -> Result<String, AIError> {
        // Use Claude 3.5 for responses without history
        self.generate_response_with_model(prompt, "claude-3-sonnet-20240229").await
    }
}