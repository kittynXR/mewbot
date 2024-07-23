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
}

#[async_trait]
impl AIProvider for AnthropicProvider {
    async fn generate_response(&self, prompt: &str) -> Result<String, AIError> {
        let response = self.client.post("https://api.anthropic.com/v1/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&json!({
                "model": "claude-2",
                "prompt": format!("Human: {}\n\nAssistant:", prompt),
                "max_tokens_to_sample": 100
            }))
            .send()
            .await
            .map_err(|e| AIError::NetworkError(e.to_string()))?;

        let response_json: serde_json::Value = response.json()
            .await
            .map_err(|e| AIError::InvalidResponse(e.to_string()))?;

        response_json["completion"]
            .as_str()
            .map(|s| s.trim().to_string())
            .ok_or_else(|| AIError::InvalidResponse("No content in response".to_string()))
    }
}