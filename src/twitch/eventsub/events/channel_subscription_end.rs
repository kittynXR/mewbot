use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use lazy_static::lazy_static;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::TwitchManager;

struct SubEndInfo {
    tier: String,
    months: u64,
}

lazy_static! {
    static ref PENDING_SUB_ENDS: Mutex<HashMap<String, SubEndInfo>> = Mutex::new(HashMap::new());
    static ref LAST_SEND_TIME: Mutex<DateTime<Utc>> = Mutex::new(Utc::now());
}

const BATCH_WINDOW: Duration = Duration::seconds(5);

pub async fn handle(
    event: &Value,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let user_name = payload["user_name"].as_str().unwrap_or("Unknown").to_string();
        let tier = payload["tier"].as_str().unwrap_or("1000").to_string();
        let months = payload["months"].as_u64().unwrap_or(0);
        let irc_client = twitch_manager.get_bot_client();

        let mut pending_subs = PENDING_SUB_ENDS.lock().await;
        pending_subs.insert(user_name, SubEndInfo { tier, months });

        let mut last_send_time = LAST_SEND_TIME.lock().await;
        if Utc::now().signed_duration_since(*last_send_time) >= BATCH_WINDOW {
            send_combined_message(pending_subs, irc_client, channel).await?;
            *last_send_time = Utc::now();
        }
    }

    Ok(())
}

async fn send_combined_message(
    mut pending_subs: tokio::sync::MutexGuard<'_, HashMap<String, SubEndInfo>>,
    irc_client: Arc<TwitchBotClient>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if pending_subs.is_empty() {
        return Ok(());
    }

    let mut tier_groups: HashMap<String, Vec<(String, u64)>> = HashMap::new();
    for (user_name, info) in pending_subs.iter() {
        tier_groups
            .entry(info.tier.clone())
            .or_insert_with(Vec::new)
            .push((user_name.clone(), info.months));
    }

    let mut messages = Vec::new();
    for (tier, users) in tier_groups {
        let tier_name = match tier.as_str() {
            "1000" => "Tier 1",
            "2000" => "Tier 2",
            "3000" => "Tier 3",
            _ => "Unknown Tier",
        };

        if users.len() == 1 {
            let (user_name, months) = &users[0];
            messages.push(format!(
                "{}'s {} sub ended after {} months. Stay amazing and cute! luv",
                user_name, tier_name, months
            ));
        } else {
            let user_list = users
                .iter()
                .map(|(name, _)| name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            messages.push(format!(
                "Multiple {} subscriptions ended: {}. See you next time! HUGGIES",
                tier_name, user_list
            ));
        }
    }

    for message in messages {
        irc_client.send_message(channel, message.as_str()).await?;
    }

    pending_subs.clear();
    Ok(())
}