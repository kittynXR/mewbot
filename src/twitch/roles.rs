use std::fmt;
use std::str::FromStr;
use std::cmp::Ordering;
use log::{debug, error};
use twitch_irc::message::Badge;
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
    badges: Option<&Vec<Badge>>,
) -> Result<UserRole, Box<dyn std::error::Error + Send + Sync>> {
    debug!("Getting user role for user_id: {}", user_id);

    // Check badges first if available
    if let Some(badges) = badges {
        for badge in badges {
            match badge.name.as_str() {
                "broadcaster" => return Ok(UserRole::Broadcaster),
                "moderator" => return Ok(UserRole::Moderator),
                "vip" => return Ok(UserRole::VIP),
                "subscriber" => return Ok(UserRole::Subscriber),
                _ => {}
            }
        }
    }

    // If no role determined from badges, fetch from UserManager
    match twitch_manager.user_manager.get_user(user_id).await {
        Ok(user) => {
            debug!("User found: {:?}", user);
            Ok(user.role)
        },
        Err(e) => {
            error!("Failed to get user: {:?}", e);
            Ok(UserRole::Viewer) // Default to Viewer if we can't determine the role
        }
    }
}