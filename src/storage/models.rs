use chrono::{DateTime, Utc};
use crate::twitch::roles::UserRole;

#[derive(Debug, Clone, Default)]
pub struct ChatterData {
    pub user_id: String,
    pub username: String,
    pub messages: Vec<String>,
    pub sentiment: f32,
    pub chatter_type: String,
    pub is_streamer: bool,
    pub stream_titles: Option<Vec<String>>,
    pub stream_categories: Option<Vec<String>>,
    pub content_summary: Option<String>,
    pub custom_notes: Option<String>,
    pub last_seen: DateTime<Utc>,
    pub role: UserRole,
}

impl ChatterData {
    pub fn new(user_id: String, username: String) -> Self {
        Self {
            user_id,
            username,
            messages: Vec::new(),
            sentiment: 0.0,
            chatter_type: "new".to_string(),
            is_streamer: false,
            stream_titles: None,
            stream_categories: None,
            content_summary: None,
            custom_notes: None,
            last_seen: Utc::now(),
            role: UserRole::Viewer,
        }
    }
}