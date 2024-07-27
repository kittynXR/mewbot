use serde_json::Value;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use lazy_static::lazy_static;

struct SubEndInfo {
    tier: String,
    months: u64,
}

lazy_static! {
    static ref PENDING_SUB_ENDS: Mutex<HashMap<String, SubEndInfo>> = Mutex::new(HashMap::new());
    static ref LAST_SEND_TIME: Mutex<Instant> = Mutex::new(Instant::now());
}

const BATCH_WINDOW: Duration = Duration::from_secs(5);

pub async fn handle(
    event: &Value,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let Some(payload) = event.get("payload").and_then(|p| p.get("event")) {
        let user_name = payload["user_name"].as_str().unwrap_or("Unknown").to_string();
        let tier = payload["tier"].as_str().unwrap_or("1000").to_string();
        let months = payload["months"].as_u64().unwrap_or(0);

        let mut pending_subs = PENDING_SUB_ENDS.lock().await;
        pending_subs.insert(user_name, SubEndInfo { tier, months });

        let mut last_send_time = LAST_SEND_TIME.lock().await;
        if last_send_time.elapsed() >= BATCH_WINDOW {
            send_combined_message(pending_subs, irc_client, channel).await?;
            *last_send_time = Instant::now();
        }
    }

    Ok(())
}

async fn send_combined_message(
    mut pending_subs: tokio::sync::MutexGuard<'_, HashMap<String, SubEndInfo>>,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
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
        irc_client.say(channel.to_string(), message).await?;
    }

    pending_subs.clear();
    Ok(())
}