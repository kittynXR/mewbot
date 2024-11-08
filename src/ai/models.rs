use std::error::Error;
use async_trait::async_trait;

#[derive(Debug)]
pub enum AIError {
    NetworkError(String),
    APIError(String),
    ParseError(String),
    InvalidResponse(String),
}

impl std::fmt::Display for AIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AIError::NetworkError(e) => write!(f, "Network error: {}", e),
            AIError::APIError(msg) => write!(f, "AI API Error: {}", msg),
            AIError::ParseError(msg) => write!(f, "AI Parse Error: {}", msg),
            AIError::InvalidResponse(e) => write!(f, "Invalid response: {}", e),
        }
    }
}


#[async_trait]
pub trait AIProvider: Send + Sync {
    async fn generate_response(&self, prompt: &str) -> Result<String, AIError>;
    async fn generate_response_without_history(&self, prompt: &str) -> Result<String, AIError>;
}


impl Error for AIError {}