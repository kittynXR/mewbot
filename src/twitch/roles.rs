use crate::storage::{ChatterData, StorageClient};
use crate::twitch::api::TwitchAPIClient;
use chrono::Utc;
use std::sync::Arc;
use tokio::sync::RwLock;

use std::fmt;
use std::str::FromStr;
use crate::twitch::role_cache::RoleCache;
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserRole {
    Viewer,
    Subscriber,
    VIP,
    Moderator,
    Broadcaster,
}

impl Default for UserRole {
    fn default() -> Self {
        UserRole::Viewer
    }
}

impl PartialOrd for UserRole {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for UserRole {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_value = match self {
            UserRole::Viewer => 0,
            UserRole::Subscriber => 1,
            UserRole::VIP => 2,
            UserRole::Moderator => 3,
            UserRole::Broadcaster => 4,
        };

        let other_value = match other {
            UserRole::Viewer => 0,
            UserRole::Subscriber => 1,
            UserRole::VIP => 2,
            UserRole::Moderator => 3,
            UserRole::Broadcaster => 4,
        };

        self_value.cmp(&other_value)
    }
}

impl fmt::Display for UserRole {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            UserRole::Viewer => write!(f, "Viewer"),
            UserRole::Subscriber => write!(f, "Subscriber"),
            UserRole::VIP => write!(f, "VIP"),
            UserRole::Moderator => write!(f, "Moderator"),
            UserRole::Broadcaster => write!(f, "Broadcaster"),
        }
    }
}

impl FromStr for UserRole {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "broadcaster" => Ok(UserRole::Broadcaster),
            "moderator" => Ok(UserRole::Moderator),
            "vip" => Ok(UserRole::VIP),
            "subscriber" => Ok(UserRole::Subscriber),
            "viewer" => Ok(UserRole::Viewer),
            _ => Err(()),
        }
    }
}

pub async fn get_user_role(
    user_id: &str,
    channel_id: &str,
    api_client: &Arc<TwitchAPIClient>,
    storage: &Arc<RwLock<StorageClient>>,
    role_cache: &Arc<RwLock<RoleCache>>,
) -> Result<UserRole, Box<dyn std::error::Error + Send + Sync>> {
    println!("Getting user role for user_id: {}", user_id);

    // Check the cache first
    if let Some(role) = role_cache.read().await.get_role(user_id) {
        println!("Role found in cache: {:?}", role);
        return Ok(role);
    }

    println!("Role not found in cache, checking database");

    // If not in cache, check the database
    let storage_read = storage.read().await;
    if let Some(chatter_data) = storage_read.get_chatter_data(user_id)? {
        let role = chatter_data.role;
        println!("Role found in database: {:?}", role);
        drop(storage_read);
        role_cache.write().await.set_role(user_id.to_string(), role.clone());
        return Ok(role);
    }
    drop(storage_read);

    println!("Role not found in database, fetching from API");

    // If not in database, fetch from API
    let role = if user_id == channel_id {
        UserRole::Broadcaster
    } else {
        match api_client.check_user_mod(channel_id, user_id).await {
            Ok(true) => UserRole::Moderator,
            Ok(false) => match api_client.check_user_vip(channel_id, user_id).await {
                Ok(true) => UserRole::VIP,
                Ok(false) => match api_client.check_user_subscription(channel_id, user_id).await {
                    Ok(true) => UserRole::Subscriber,
                    Ok(false) => UserRole::Viewer,
                    Err(e) => {
                        println!("Error checking subscription status: {:?}", e);
                        UserRole::Viewer
                    }
                },
                Err(e) => {
                    println!("Error checking VIP status: {:?}", e);
                    UserRole::Viewer
                }
            },
            Err(e) => {
                println!("Error checking moderator status: {:?}", e);
                UserRole::Viewer
            }
        }
    };

    println!("Role fetched from API: {:?}", role);

    // Update the database and cache
    {
        let mut storage_write = storage.write().await;
        let mut chatter_data = ChatterData::new(user_id.to_string(), user_id.to_string());
        chatter_data.role = role.clone();
        chatter_data.last_seen = Utc::now();
        if let Err(e) = storage_write.upsert_chatter(&chatter_data) {
            println!("Error upserting chatter data: {:?}", e);
        } else {
            println!("Chatter data updated in database");
        }
    }

    role_cache.write().await.set_role(user_id.to_string(), role.clone());
    println!("Role updated in cache");

    Ok(role)
}