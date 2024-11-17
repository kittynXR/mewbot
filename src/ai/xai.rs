use async_trait::async_trait;
use super::models::{AIProvider, AIError};
use reqwest::Client;
use serde_json::json;
use log::{debug, error};

pub struct XAIProvider {
    api_key: String,
    client: Client,
}

impl XAIProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: Client::new(),
        }
    }

    async fn generate_response_with_model(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        temperature: Option<f32>
    ) -> Result<String, AIError> {
        debug!("Generating XAI response with prompt: {}", prompt);

        let mut messages = Vec::new();

        if let Some(system_content) = system_prompt {
            messages.push(json!({
                "role": "system",
                "content": system_content
            }));
        }

        messages.push(json!({
            "role": "user",
            "content": prompt
        }));

        let request_body = json!({
            "messages": messages,
            "model": "grok-beta",
            "stream": false,
            "temperature": temperature.unwrap_or(0.7)
        });

        let response = self.client
            .post("https://api.x.ai/v1/chat/completions")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| AIError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let error_text = response.text().await
                .unwrap_or_else(|e| format!("Failed to get error text: {}", e));
            error!("XAI API error: {}", error_text);
            return Err(AIError::APIError(error_text));
        }

        let response_json: serde_json::Value = response.json()
            .await
            .map_err(|e| AIError::InvalidResponse(e.to_string()))?;

        response_json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.trim().to_string())
            .ok_or_else(|| AIError::InvalidResponse("No content in response".to_string()))
    }

    pub async fn generate_grok_response(&self, prompt: &str) -> Result<String, AIError> {
        const SYSTEM_PROMPT: &str = "You are Grok, an AI assistant who can access real-time data from the web and X. \
                                   Provide clear, informative, and occasionally witty responses based on current information.";

        self.generate_response_with_model(
            prompt,
            Some(SYSTEM_PROMPT),
            Some(0.7)
        ).await
    }
}

#[async_trait]
impl AIProvider for XAIProvider {
    async fn generate_response(&self, prompt: &str) -> Result<String, AIError> {
        self.generate_response_with_model(prompt, None, None).await
    }

    async fn generate_response_without_history(&self, prompt: &str) -> Result<String, AIError> {
        self.generate_response_with_model(prompt, None, None).await
    }
}