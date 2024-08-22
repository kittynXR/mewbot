use std::fmt;
use std::str::FromStr;
use std::cmp::Ordering;
use log::{debug, error};
use crate::twitch::manager::TwitchManager;

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
    twitch_manager: &TwitchManager,
) -> Result<UserRole, Box<dyn std::error::Error + Send + Sync>> {
    debug!("Getting user role for user_id: {}", user_id);

    // Get user from TwitchManager
    let user = twitch_manager.user_manager.get_user(user_id).await?;

    // If the user has a role, return it
    if user.role != UserRole::Viewer {
        debug!("Role found for user: {:?}", user.role);
        return Ok(user.role);
    }

    debug!("No specific role found, checking with Twitch API");

    // If the user doesn't have a specific role, check with the Twitch API
    let channel_id = twitch_manager.api_client.get_broadcaster_id().await?;

    let role = if user_id == channel_id {
        UserRole::Broadcaster
    } else {
        match twitch_manager.api_client.check_user_mod(&channel_id, user_id).await {
            Ok(true) => UserRole::Moderator,
            Ok(false) => match twitch_manager.api_client.check_user_vip(&channel_id, user_id).await {
                Ok(true) => UserRole::VIP,
                Ok(false) => match twitch_manager.api_client.check_user_subscription(&channel_id, user_id).await {
                    Ok(true) => UserRole::Subscriber,
                    Ok(false) => UserRole::Viewer,
                    Err(e) => {
                        error!("Error checking subscription status: {:?}", e);
                        UserRole::Viewer
                    }
                },
                Err(e) => {
                    error!("Error checking VIP status: {:?}", e);
                    UserRole::Viewer
                }
            },
            Err(e) => {
                error!("Error checking moderator status: {:?}", e);
                UserRole::Viewer
            }
        }
    };

    debug!("Role fetched from API: {:?}", role);

    // Update the user's role in the TwitchManager
    if let Err(e) = twitch_manager.user_manager.update_user_role(user_id, role.clone()).await {
        error!("Error updating user role: {:?}", e);
    } else {
        debug!("User role updated in TwitchManager");
    }

    Ok(role)
}