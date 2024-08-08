use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct WebUIConfig {
    pub port: u16,
    // Add other web UI specific configurations here
}

impl Default for WebUIConfig {
    fn default() -> Self {
        WebUIConfig {
            port: 3333,
        }
    }
}