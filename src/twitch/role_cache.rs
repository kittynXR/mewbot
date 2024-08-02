use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::twitch::roles::UserRole;

pub struct RoleCache {
    roles: HashMap<String, (UserRole, DateTime<Utc>)>,
}

impl RoleCache {
    pub fn new() -> Self {
        RoleCache {
            roles: HashMap::new(),
        }
    }

    pub fn get_role(&self, user_id: &str) -> Option<UserRole> {
        self.roles.get(user_id).and_then(|(role, timestamp)| {
            if Utc::now().signed_duration_since(*timestamp).num_hours() < 24 {
                Some(role.clone())
            } else {
                None
            }
        })
    }

    pub fn set_role(&mut self, user_id: String, role: UserRole) {
        self.roles.insert(user_id, (role, Utc::now()));
    }

    pub fn clear(&mut self) {
        self.roles.clear();
        println!("Role cache cleared");
    }
}