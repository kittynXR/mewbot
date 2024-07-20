use async_trait::async_trait;

#[derive(Debug)]
pub enum AIError {
    NetworkError(String),
    APIError(String),
    InvalidResponse(String),
}

impl std::fmt::Display for AIError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AIError::NetworkError(e) => write!(f, "Network error: {}", e),
            AIError::APIError(e) => write!(f, "API error: {}", e),
            AIError::InvalidResponse(e) => write!(f, "Invalid response: {}", e),
        }
    }
}

#[async_trait]
pub trait AIProvider: Send + Sync {
    async fn generate_response(&self, prompt: &str) -> Result<String, AIError>;
}