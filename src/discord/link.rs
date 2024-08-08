// src/discord/link.rs

use serenity::model::id::UserId;
use std::collections::HashMap;
use tokio::sync::RwLock;

pub struct UserLinks {
    discord_to_twitch: RwLock<HashMap<UserId, String>>,
    twitch_to_discord: RwLock<HashMap<String, UserId>>,
    pending_verifications: RwLock<HashMap<u32, UserId>>, // Verification code to Discord UserId
}

impl UserLinks {
    pub fn new() -> Self {
        Self {
            discord_to_twitch: RwLock::new(HashMap::new()),
            twitch_to_discord: RwLock::new(HashMap::new()),
            pending_verifications: RwLock::new(HashMap::new()),
        }
    }

    pub async fn add_pending_verification(&self, discord_id: UserId, code: u32) -> Result<(), &'static str> {
        let mut pending = self.pending_verifications.write().await;
        pending.insert(code, discord_id);
        Ok(())
    }

    pub async fn verify_and_link(&self, twitch_username: &str, code: u32) -> Result<UserId, &'static str> {
        let mut pending = self.pending_verifications.write().await;
        if let Some(discord_id) = pending.remove(&code) {
            let mut discord_to_twitch = self.discord_to_twitch.write().await;
            let mut twitch_to_discord = self.twitch_to_discord.write().await;

            discord_to_twitch.insert(discord_id, twitch_username.to_string());
            twitch_to_discord.insert(twitch_username.to_string(), discord_id);

            Ok(discord_id)
        } else {
            Err("Invalid verification code or no pending verification found")
        }
    }

    pub async fn get_discord_id(&self, twitch_username: &str) -> Option<UserId> {
        let twitch_to_discord = self.twitch_to_discord.read().await;
        twitch_to_discord.get(twitch_username).cloned()
    }

    pub async fn get_twitch_username(&self, discord_id: UserId) -> Option<String> {
        let discord_to_twitch = self.discord_to_twitch.read().await;
        discord_to_twitch.get(&discord_id).cloned()
    }
}