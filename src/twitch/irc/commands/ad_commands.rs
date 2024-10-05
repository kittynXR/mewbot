use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use tokio::time::sleep;
use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;

pub struct AdInfo {
    pub text: String,
    pub interval: Duration,
    pub end_time: DateTime<Utc>,
}

pub struct AdManager {
    pub ads: HashMap<String, AdInfo>,
}

impl Default for AdManager {
    fn default() -> Self {
        Self::new()
    }
}


impl AdManager {
    pub fn new() -> Self {
        Self {
            ads: HashMap::new(),
        }
    }

    pub fn add_ad(&mut self, id: String, text: String, interval: Duration, duration: Duration) {
        self.ads.insert(id, AdInfo {
            text,
            interval,
            end_time: Utc::now() + duration,
        });
    }

    pub fn remove_ad(&mut self, id: &str) -> bool {
        self.ads.remove(id).is_some()
    }

    pub fn clear_ads(&mut self) {
        self.ads.clear()
    }

    pub fn get_active_ads(&self) -> Vec<(String, &AdInfo)> {
        let now = Utc::now();
        self.ads
            .iter()
            .filter(|(_, info)| info.end_time > now)
            .map(|(id, info)| (id.clone(), info))
            .collect()
    }
}

pub struct StartAdCommand;

#[async_trait::async_trait]
impl Command for StartAdCommand {
    fn name(&self) -> &'static str {
        "!startad"
    }

    fn description(&self) -> &'static str {
        "Starts a scheduled ad. Usage: !startad <interval_minutes> <duration_minutes> <ad_text>"
    }

    fn required_role(&self) -> UserRole {
        UserRole::Moderator
    }

    async fn execute(&self, ctx: &CommandContext, args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if args.len() < 3 {
            ctx.bot_client.send_message(&ctx.channel, "Usage: !startad <interval_minutes> <duration_minutes> <ad_text>").await?;
            return Ok(());
        }

        let interval_minutes: i64 = args[0].parse()?;
        let duration_minutes: i64 = args[1].parse()?;
        let ad_text = args[2..].join(" ");

        let ad_manager = ctx.twitch_manager.get_ad_manager();
        let mut ad_manager_lock = ad_manager.write().await;
        let ad_id = format!("ad_{}", Utc::now().timestamp());
        ad_manager_lock.add_ad(
            ad_id.clone(),
            ad_text.clone(),
            Duration::minutes(interval_minutes),
            Duration::minutes(duration_minutes),
        );
        drop(ad_manager_lock);

        // Start the ad loop
        let bot_client = ctx.bot_client.clone();
        let channel = ctx.channel.clone();
        let ad_manager_clone = ad_manager.clone();
        tokio::spawn(async move {
            loop {
                sleep(std::time::Duration::from_secs((interval_minutes * 60) as u64)).await;
                let ad_manager_lock = ad_manager_clone.read().await;
                if let Some(ad_info) = ad_manager_lock.ads.get(&ad_id) {
                    if Utc::now() < ad_info.end_time {
                        bot_client.send_message(&channel, &ad_info.text).await.ok();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
                drop(ad_manager_lock);
            }
        });

        ctx.bot_client.send_message(&ctx.channel, &format!("Ad started: will run every {} minutes for {} minutes", interval_minutes, duration_minutes)).await?;
        Ok(())
    }
}

pub struct StopAdsCommand;

#[async_trait::async_trait]
impl Command for StopAdsCommand {
    fn name(&self) -> &'static str {
        "!stopads"
    }

    fn description(&self) -> &'static str {
        "Stops all running ads"
    }

    fn required_role(&self) -> UserRole {
        UserRole::Moderator
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let ad_manager = ctx.twitch_manager.get_ad_manager();
        let mut ad_manager_lock = ad_manager.write().await;
        ad_manager_lock.clear_ads();
        drop(ad_manager_lock);

        ctx.bot_client.send_message(&ctx.channel, "All ads have been stopped.").await?;
        Ok(())
    }
}