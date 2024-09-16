use crate::twitch::TwitchAPIClient;
use std::sync::Arc;
use crate::twitch::models::RedeemInfo;

pub struct RedeemSyncManager {
    api_client: Arc<TwitchAPIClient>,
}

impl RedeemSyncManager {
    pub fn new(api_client: Arc<TwitchAPIClient>) -> Self {
        Self { api_client }
    }

    pub async fn fetch_all_redeems(&self) -> Result<Vec<RedeemInfo>, Box<dyn std::error::Error + Send + Sync>> {
        let rewards = self.api_client.get_channel_point_rewards().await?;
        Ok(rewards
            .into_iter()
            .map(|reward| RedeemInfo {
                id: Some(reward.id),
                title: reward.title,
                cost: reward.cost,
                is_enabled: reward.is_enabled,
                prompt: reward.prompt,
                cooldown: reward.cooldown_seconds,
                is_global_cooldown: reward.global_cooldown.map(|gc| gc.is_enabled).unwrap_or(false),
                limit_per_stream: reward.max_per_stream.map(|mps| mps.max_per_stream),
                limit_per_user: reward.max_per_user_per_stream.map(|mpups| mpups.max_per_user_per_stream),
                use_osc: false, // This information isn't available from the API, so we'll need to merge with local config
                osc_config: None,
                enabled_games: Vec::new(),
                disabled_games: Vec::new(),
                enabled_offline: false,
                is_conflicting: false,
                user_input_required: reward.is_user_input_required,
                auto_complete: false,
            })
            .collect())
    }

    pub async fn create_redeem(&self, info: &RedeemInfo) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let reward = self.api_client.create_custom_reward(
            &info.title,
            info.cost,
            info.is_enabled,
            info.cooldown.unwrap_or(0),
            &info.prompt,
            info.user_input_required,
        ).await?;
        Ok(reward.id)
    }

    pub async fn update_redeem(&self, info: &RedeemInfo) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(id) = &info.id {
            self.api_client.update_custom_reward(
                id,
                &info.title,
                info.cost,
                info.is_enabled,
                info.cooldown.unwrap_or(0),
                &info.prompt,
                info.user_input_required,
            ).await?;
            Ok(())
        } else {
            Err("Redeem ID not found".into())
        }
    }

    pub async fn delete_redeem(&self, id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let broadcaster_id = self.api_client.get_broadcaster_id().await?;
        self.api_client.delete_custom_reward(&broadcaster_id, id).await
    }
}