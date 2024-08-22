use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};
use log::{debug, error, info};
use twitch_irc::message::PrivmsgMessage;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::api::requests::shoutout::send_shoutout;
use crate::twitch::redeems::RedeemManager;
use tokio::sync::RwLock;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use crate::twitch::TwitchManager;

pub struct ShoutoutCooldown {
    global: Instant,
    per_user: HashMap<String, Instant>,
}

impl ShoutoutCooldown {
    pub fn new() -> Self {
        ShoutoutCooldown {
            global: Instant::now() - Duration::from_secs(121), // Initialize as if cooldown has passed
            per_user: HashMap::new(),
        }
    }
}

fn strip_at_symbol(username: &str) -> &str {
    username.strip_prefix('@').unwrap_or(username)
}

async fn is_user_moderator(api_client: Arc<TwitchAPIClient>, broadcaster_id: &str, user_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    api_client.is_user_moderator(user_id).await
}

pub async fn handle_shoutout(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
    cooldowns: &Arc<Mutex<ShoutoutCooldown>>,
    params: &[&str],
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if params.is_empty() {
        client.send_message(channel, "Please specify a user to shoutout!").await?;
        return Ok(());
    }

    let twitch_api_client = twitch_manager.get_api_client();
    let target = strip_at_symbol(params[0]);
    debug!("Shoutout target: {}", target);

    // Check if the user is trying to shout themselves out
    if target.to_lowercase() == msg.sender.name.to_lowercase() {
        let self_shoutout_message = format!("@{}, you're already awesome! No need to shout yourself out! pepeStepBro pepeStepBro ", msg.sender.name);
        client.send_message(channel, &self_shoutout_message).await?;
        return Ok(());
    }


    // Check if the target is the broadcaster
    if target.to_lowercase() == channel.to_lowercase() {
        let broadcaster_shoutout_message = format!("@{}, 推し～！ Cannot shout out our oshi {}! They're already here blessing us with their sugoi presence! ٩(◕‿◕｡)۶", msg.sender.name, channel);
        client.send_message(channel, &broadcaster_shoutout_message).await?;
        return Ok(());
    }

    let mut cooldowns = cooldowns.lock().await;

    let now = Instant::now();
    let (global_cooldown_passed, user_cooldown_passed) = {
        let global_passed = now.duration_since(cooldowns.global) >= Duration::from_secs(120);
        let user_passed = cooldowns.per_user.get(target)
            .map_or(true, |&last_use| now.duration_since(last_use) >= Duration::from_secs(3600));

        (global_passed, user_passed)
    };

    // Check cooldowns and send messages if necessary
    if !global_cooldown_passed {
        let remaining = Duration::from_secs(120) - now.duration_since(cooldowns.global);
        client.send_message(channel, &format!("Shoutout is on global cooldown. Please wait {} seconds.", remaining.as_secs())).await?;
        return Ok(());
    }

    if !user_cooldown_passed {
        let user_last_use = cooldowns.per_user.get(target).cloned().unwrap_or(now - Duration::from_secs(3601));
        let remaining = Duration::from_secs(3600) - now.duration_since(user_last_use);
        client.send_message(channel, &format!("Cannot shoutout {} again so soon. Please wait {} minutes.", target, remaining.as_secs() / 60)).await?;
        return Ok(());
    }

    // If cooldowns have passed, update them
    if global_cooldown_passed && user_cooldown_passed {
        cooldowns.global = now;
        cooldowns.per_user.insert(target.to_string(), now);
    }

    // Continue with the rest of the function...

    // Continue with the rest of the function...

    // Check if the stream is live
    let redeem_manager_read = redeem_manager.read().await;
    let stream_status = redeem_manager_read.stream_status.read().await;
    if !stream_status.is_live {
        client.send_message(channel, &format!("Sorry, @{}, shoutouts can only be given when the stream is live.", msg.sender.name)).await?;
        return Ok(());
    }
    drop(stream_status);
    drop(redeem_manager_read);

    // Perform shoutout
    match twitch_api_client.get_user_info(target).await {
        Ok(user_info) => {
            let to_broadcaster_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get user ID")?.to_string();
            let broadcaster_id = twitch_api_client.get_broadcaster_id().await?;

            // Check if the sender is a moderator
            let is_mod = is_user_moderator(twitch_api_client, &broadcaster_id, &msg.sender.id).await?;

            let moderator_id = if is_mod {
                msg.sender.id.clone()
            } else {
                broadcaster_id.clone() // Use broadcaster ID if the sender is not a mod
            };

            match send_shoutout(twitch_manager, &broadcaster_id, &moderator_id, &to_broadcaster_id).await {
                Ok(_) => {
                    let message = format!("Go check out @{}! They were last streaming something awesome. Give them a follow at https://twitch.tv/{}",
                                          target,
                                          target
                    );
                    client.send_message(channel, &message).await?;

                    // Update cooldowns
                    cooldowns.global = now;
                    cooldowns.per_user.insert(target.to_string(), now);
                },
                Err(e) => {
                    error!("Error sending shoutout: {}", e);
                    client.send_message(channel, &format!("Sorry, @{}, I couldn't send a shoutout to {}. There was an API error.", msg.sender.name, target)).await?;
                }
            }
        },
        Err(e) => {
            error!("Error getting user info for shoutout target: {}", e);
            client.send_message(channel, &format!("Sorry, I couldn't find information for user {}.", target)).await?;
        }
    }

    Ok(())
}