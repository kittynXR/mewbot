use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use twitch_irc::message::PrivmsgMessage;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::TwitchManager;
use crate::storage::StorageClient;
use crate::discord::UserLinks;

pub struct AdInfo {
    pub text: String,
    pub interval: Duration,
    pub end_time: Instant,
}

pub struct AdManager {
    pub ads: HashMap<String, AdInfo>,
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
            end_time: Instant::now() + duration,
        });
    }

    pub fn remove_ad(&mut self, id: &str) -> bool {
        self.ads.remove(id).is_some()
    }

    pub fn clear_ads(&mut self) {
        self.ads.clear();
    }

    pub fn get_active_ads(&self) -> Vec<(String, &AdInfo)> {
        self.ads
            .iter()
            .filter(|(_, info)| info.end_time > Instant::now())
            .map(|(id, info)| (id.clone(), info))
            .collect()
    }
}

pub async fn handle_startad(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
    storage: &Arc<RwLock<StorageClient>>,
    user_links: &Arc<UserLinks>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if params.len() < 3 {
        client.send_message(channel, "Usage: !startad <interval_minutes> <duration_minutes> <ad_text>").await?;
        return Ok(());
    }

    let interval_minutes: u64 = params[0].parse()?;
    let duration_minutes: u64 = params[1].parse()?;
    let ad_text = params[2..].join(" ");

    let ad_manager = twitch_manager.get_ad_manager();
    let mut ad_manager_lock = ad_manager.write().await;
    let ad_id = format!("ad_{}", Instant::now().elapsed().as_secs());
    ad_manager_lock.add_ad(
        ad_id.clone(),
        ad_text.clone(),
        Duration::from_secs(interval_minutes * 60),
        Duration::from_secs(duration_minutes * 60),
    );
    drop(ad_manager_lock);

    // Start the ad loop
    let client_clone = client.clone();
    let channel_clone = channel.to_string();
    let ad_manager_clone = ad_manager.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(interval_minutes * 60)).await;
            let ad_manager_lock = ad_manager_clone.read().await;
            if let Some(ad_info) = ad_manager_lock.ads.get(&ad_id) {
                if Instant::now() < ad_info.end_time {
                    client_clone.send_message(&channel_clone, &ad_info.text).await.ok();
                } else {
                    break;
                }
            } else {
                break;
            }
            drop(ad_manager_lock);
        }
    });

    client.send_message(channel, &format!("Ad started: will run every {} minutes for {} minutes", interval_minutes, duration_minutes)).await?;
    Ok(())
}

pub async fn handle_stopads(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
    storage: &Arc<RwLock<StorageClient>>,
    user_links: &Arc<UserLinks>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ad_manager = twitch_manager.get_ad_manager();
    let mut ad_manager_lock = ad_manager.write().await;
    ad_manager_lock.clear_ads();
    drop(ad_manager_lock);

    client.send_message(channel, "All ads have been stopped.").await?;
    Ok(())
}