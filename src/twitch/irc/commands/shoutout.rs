use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::{Duration, Instant};
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::api::requests::shoutout::send_shoutout;
use crate::twitch::redeems::RedeemManager;
use tokio::sync::RwLock;
use crate::storage::StorageClient;
use crate::discord::UserLinks;

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

async fn is_user_moderator(api_client: &TwitchAPIClient, broadcaster_id: &str, user_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    api_client.is_user_moderator(user_id).await
}

pub async fn handle_shoutout(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    api_client: &Arc<TwitchAPIClient>,
    cooldowns: &Arc<Mutex<ShoutoutCooldown>>,
    params: &[&str],
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if params.is_empty() {
        client.say(channel.to_string(), "Please specify a user to shoutout!".to_string()).await?;
        return Ok(());
    }

    let target = strip_at_symbol(params[0]);
    println!("Shoutout target: {}", target);

    // Check if the user is trying to shout themselves out
    if target.to_lowercase() == msg.sender.name.to_lowercase() {
        let self_shoutout_message = format!("@{}, you're already awesome! No need to shout yourself out! pepeStepBro pepeStepBro ", msg.sender.name);
        client.say(channel.to_string(), self_shoutout_message).await?;
        return Ok(());
    }


    // Check if the target is the broadcaster
    if target.to_lowercase() == channel.to_lowercase() {
        let broadcaster_shoutout_message = format!("@{}, 推し～！ Cannot shout out our oshi {}! They're already here blessing us with their sugoi presence! ٩(◕‿◕｡)۶", msg.sender.name, channel);
        client.say(channel.to_string(), broadcaster_shoutout_message).await?;
        return Ok(());
    }

    let mut cooldowns = cooldowns.lock().await;
    let now = Instant::now();

    // Check global cooldown
    if now.duration_since(cooldowns.global) < Duration::from_secs(120) {
        let remaining = Duration::from_secs(120) - now.duration_since(cooldowns.global);
        client.say(channel.to_string(), format!("Shoutout is on global cooldown. Please wait {} seconds.", remaining.as_secs())).await?;
        return Ok(());
    }

    // Check per-user cooldown
    if let Some(last_use) = cooldowns.per_user.get(target) {
        if now.duration_since(*last_use) < Duration::from_secs(3600) {
            let remaining = Duration::from_secs(3600) - now.duration_since(*last_use);
            client.say(channel.to_string(), format!("Cannot shoutout {} again so soon. Please wait {} minutes.", target, remaining.as_secs() / 60)).await?;
            return Ok(());
        }
    }

    // Check if the stream is live
    let redeem_manager_read = redeem_manager.read().await;
    let stream_status = redeem_manager_read.stream_status.read().await;
    if !stream_status.is_live {
        client.say(channel.to_string(), format!("Sorry, @{}, shoutouts can only be given when the stream is live.", msg.sender.name)).await?;
        return Ok(());
    }
    drop(stream_status);
    drop(redeem_manager_read);

    // Perform shoutout
    match api_client.get_user_info(target).await {
        Ok(user_info) => {
            let to_broadcaster_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get user ID")?.to_string();
            let broadcaster_id = api_client.get_broadcaster_id().await?;

            // Check if the sender is a moderator
            let is_mod = is_user_moderator(api_client, &broadcaster_id, &msg.sender.id).await?;

            let moderator_id = if is_mod {
                msg.sender.id.clone()
            } else {
                broadcaster_id.clone() // Use broadcaster ID if the sender is not a mod
            };

            match send_shoutout(api_client, &broadcaster_id, &moderator_id, &to_broadcaster_id).await {
                Ok(_) => {
                    let message = format!("Go check out @{}! They were last streaming something awesome. Give them a follow at https://twitch.tv/{}",
                                          target,
                                          target
                    );
                    client.say(channel.to_string(), message).await?;

                    // Update cooldowns
                    cooldowns.global = now;
                    cooldowns.per_user.insert(target.to_string(), now);
                },
                Err(e) => {
                    eprintln!("Error sending shoutout: {}", e);
                    client.say(channel.to_string(), format!("Sorry, @{}, I couldn't send a shoutout to {}. There was an API error.", msg.sender.name, target)).await?;
                }
            }
        },
        Err(e) => {
            println!("Error getting user info for shoutout target: {}", e);
            client.say(channel.to_string(), format!("Sorry, I couldn't find information for user {}.", target)).await?;
        }
    }

    Ok(())
}