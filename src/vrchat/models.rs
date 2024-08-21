use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use chrono::{DateTime, Utc};

#[derive(Debug)]
pub struct VRChatError(pub String);

impl fmt::Display for VRChatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for VRChatError {}

impl From<Box<dyn Error + Send + Sync>> for VRChatError {
    fn from(error: Box<dyn Error + Send + Sync>) -> Self {
        VRChatError(error.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct World {
    pub id: String,
    pub name: String,
    #[serde(rename = "authorName")]
    pub author_name: String,
    pub capacity: i32,
    pub description: String,
    #[serde(rename = "releaseStatus")]
    pub release_status: String,
    #[serde(rename = "created_at")]
    pub created_at: DateTime<Utc>,
    #[serde(rename = "updated_at")]
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VRChatMessage {
    Error(ErrorMessage),
    UserLocation(serde_json::Value),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub err: String,
    #[serde(rename = "authToken")]
    pub auth_token: String,
    pub ip: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Friend {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub status: String,
    pub location: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VRChatStatus {
    pub online: bool,
    pub current_world: Option<World>,
    pub friend_count: usize,
}