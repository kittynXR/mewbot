mod models;
mod client;
mod openai;
mod anthropic;
mod xai;
mod web_search;

pub use models::{AIProvider, AIError};
pub use client::AIClient;
pub use web_search::{WebSearchClient, WebSearchResult, SearchProvider};