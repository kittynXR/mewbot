use std::collections::{HashMap, VecDeque};
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
use crate::ai::AIClient;

const GLOBAL_COOLDOWN_SECONDS: u64 = 121; // 2 minutes
const USER_COOLDOWN_SECONDS: u64 = 3600; // 1 hour

struct ShoutoutQueueItem {
    target: String,
    requester: String,
    enqueue_time: Instant,
}

struct ShoutoutQueue {
    queue: VecDeque<ShoutoutQueueItem>,
}

impl ShoutoutQueue {
    fn new() -> Self {
        ShoutoutQueue {
            queue: VecDeque::new(),
        }
    }

    fn enqueue(&mut self, target: String, requester: String) {
        self.queue.push_back(ShoutoutQueueItem {
            target,
            requester,
            enqueue_time: Instant::now(),
        });
    }

    fn dequeue(&mut self) -> Option<ShoutoutQueueItem> {
        self.queue.pop_front()
    }

    fn position(&self, target: &str) -> Option<usize> {
        self.queue.iter().position(|item| item.target == target)
    }
}

pub struct ShoutoutCooldown {
    global: Instant,
    per_user: HashMap<String, Instant>,
    queue: ShoutoutQueue,
}

impl ShoutoutCooldown {
    pub fn new() -> Self {
        ShoutoutCooldown {
            global: Instant::now() - Duration::from_secs(GLOBAL_COOLDOWN_SECONDS + 1),
            per_user: HashMap::new(),
            queue: ShoutoutQueue::new(),
        }
    }

    pub fn check_cooldowns(&self, target: &str) -> (bool, bool) {
        let now = Instant::now();
        let global_passed = now.duration_since(self.global) >= Duration::from_secs(GLOBAL_COOLDOWN_SECONDS);
        let user_passed = self.per_user.get(target)
            .map_or(true, |&last_use| now.duration_since(last_use) >= Duration::from_secs(USER_COOLDOWN_SECONDS));

        (global_passed, user_passed)
    }

    pub fn update_cooldowns(&mut self, target: &str) {
        let now = Instant::now();
        self.global = now;
        self.per_user.insert(target.to_string(), now);
    }

    pub fn get_remaining_cooldown(&self, target: &str) -> (Option<Duration>, Option<Duration>) {
        let now = Instant::now();
        let global_remaining = if now.duration_since(self.global) < Duration::from_secs(GLOBAL_COOLDOWN_SECONDS) {
            Some(Duration::from_secs(GLOBAL_COOLDOWN_SECONDS) - now.duration_since(self.global))
        } else {
            None
        };

        let user_remaining = self.per_user.get(target).and_then(|&last_use| {
            if now.duration_since(last_use) < Duration::from_secs(USER_COOLDOWN_SECONDS) {
                Some(Duration::from_secs(USER_COOLDOWN_SECONDS) - now.duration_since(last_use))
            } else {
                None
            }
        });

        (global_remaining, user_remaining)
    }

    pub fn enqueue_shoutout(&mut self, target: String, requester: String) {
        self.queue.enqueue(target, requester);
    }

    pub fn dequeue_shoutout(&mut self) -> Option<ShoutoutQueueItem> {
        self.queue.dequeue()
    }

    pub fn get_queue_position(&self, target: &str) -> Option<usize> {
        self.queue.position(target)
    }
}

pub async fn handle_shoutout(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
    cooldowns: &Arc<Mutex<ShoutoutCooldown>>,
    params: &[&str],
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    storage: &Arc<RwLock<StorageClient>>,
    user_links: &Arc<UserLinks>,
    ai_client: &Option<Arc<AIClient>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if params.is_empty() {
        client.send_message(channel, "Please specify a user to shoutout!").await?;
        return Ok(());
    }

    let target_username = strip_at_symbol(params[0]);
    debug!("Shoutout target: {}", target_username);

    // Check if the user is trying to shout themselves out or the broadcaster
    if target_username.to_lowercase() == msg.sender.name.to_lowercase() || target_username.to_lowercase() == channel.to_lowercase() {
        let message = if target_username.to_lowercase() == msg.sender.name.to_lowercase() {
            format!("@{}, you're already awesome! No need to shout yourself out! pepeStepBro pepeStepBro", msg.sender.name)
        } else {
            format!("@{}, 推し～！ Cannot shout out our oshi {}! They're already here blessing us with their sugoi presence! ٩(◕‿◕｡)۶", msg.sender.name, channel)
        };
        client.send_message(channel, &message).await?;
        return Ok(());
    }

    // Check cooldowns and handle queue
    let mut cooldowns = cooldowns.lock().await;
    let (global_cooldown_passed, user_cooldown_passed) = cooldowns.check_cooldowns(target_username);

    if !user_cooldown_passed {
        if let (_, Some(remaining)) = cooldowns.get_remaining_cooldown(target_username) {
            client.send_message(channel, &format!("Cannot shoutout {} again so soon. Please wait {} minutes.", target_username, remaining.as_secs() / 60)).await?;
            return Ok(());
        }
    }

    if !global_cooldown_passed {
        cooldowns.enqueue_shoutout(target_username.to_string(), msg.sender.name.clone());
        let position = cooldowns.get_queue_position(target_username).unwrap_or(0) + 1;
        client.send_message(channel, &format!("Shoutout for {} has been added to the queue. Current position: {}", target_username, position)).await?;
        return Ok(());
    }

    // Process the shoutout
    cooldowns.update_cooldowns(target_username);
    drop(cooldowns);

    process_shoutout(client, channel, twitch_manager, target_username, ai_client).await?;

    Ok(())
}

async fn process_shoutout(
    client: &Arc<TwitchBotClient>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
    target_username: &str,
    ai_client: &Option<Arc<AIClient>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let twitch_api_client = twitch_manager.get_api_client();

    match twitch_api_client.get_user_info(target_username).await {
        Ok(user_info) => {
            let user_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get user ID")?.to_string();
            let display_name = user_info["data"][0]["display_name"].as_str().unwrap_or(target_username);
            let login = user_info["data"][0]["login"].as_str().unwrap_or(target_username);

            if let Err(e) = twitch_manager.update_streamer_data(&user_id).await {
                error!("Failed to update streamer data: {}", e);
            }

            let shoutout_message = match ai_client {
                Some(ai) => generate_ai_shoutout_message(twitch_manager, ai, &user_id).await?,
                None => generate_simple_shoutout_message(twitch_manager, &user_id).await?,
            };

            client.send_message(channel, &shoutout_message).await?;

            if twitch_manager.is_stream_live().await {
                let broadcaster_id = twitch_api_client.get_broadcaster_id().await?;
                let moderator_id = broadcaster_id.clone(); // Assuming the bot is always a moderator

                if let Err(e) = send_shoutout(twitch_manager, &broadcaster_id, &moderator_id, &user_id).await {
                    error!("Error sending API shoutout: {}", e);
                }
            }
        },
        Err(e) => {
            error!("Error getting user info for shoutout target: {}", e);
            client.send_message(channel, &format!("Sorry, I couldn't find information for user {}.", target_username)).await?;
        }
    }

    Ok(())
}

pub async fn start_shoutout_queue_processor(
    client: Arc<TwitchBotClient>,
    channel: String,
    twitch_manager: Arc<TwitchManager>,
    cooldowns: Arc<Mutex<ShoutoutCooldown>>,
    ai_client: Option<Arc<AIClient>>,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(GLOBAL_COOLDOWN_SECONDS)).await;

            let mut cooldowns = cooldowns.lock().await;
            if let Some(item) = cooldowns.dequeue_shoutout() {
                drop(cooldowns);
                if let Err(e) = process_shoutout(&client, &channel, &twitch_manager, &item.target, &ai_client).await {
                    error!("Error processing queued shoutout: {}", e);
                }
            } else {
                drop(cooldowns);
            }
        }
    });
}

fn strip_at_symbol(username: &str) -> &str {
    username.strip_prefix('@').unwrap_or(username)
}

async fn is_user_moderator(api_client: Arc<TwitchAPIClient>, broadcaster_id: &str, user_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    api_client.is_user_moderator(user_id).await
}

async fn generate_ai_shoutout_message(
    twitch_manager: &Arc<TwitchManager>,
    ai_client: &Arc<AIClient>,
    user_id: &str
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut user = twitch_manager.get_user(user_id).await?;

    // Double-check user information
    if user.display_name == "displayname" || user.username == user_id {
        // If the display name is still "displayname" or the username is the user ID,
        // try to update the user information again
        twitch_manager.update_streamer_data(user_id).await?;
        // Fetch the updated user information
        let updated_user = twitch_manager.get_user(user_id).await?;
        if updated_user.display_name != "displayname" && updated_user.username != user_id {
            // Use the updated user information if it's valid
            user = updated_user;
        } else {
            // If we still can't get valid information, return an error
            return Err("Unable to fetch valid user information for shoutout".into());
        }
    }

    if let Some(streamer_data) = &user.streamer_data {
        let mut prompt = format!(
            "Generate an engaging and friendly Twitch shoutout message for streamer @{} with the following information:\n",
            user.display_name
        );

        if !streamer_data.recent_games.is_empty() {
            prompt.push_str(&format!("- Last played game: {}\n", streamer_data.recent_games[0]));
        }

        if !streamer_data.current_title.is_empty() {
            prompt.push_str(&format!("- Last stream title: \"{}\"\n", streamer_data.current_title));
        }

        if !streamer_data.current_tags.is_empty() {
            prompt.push_str(&format!("- Stream tags: {}\n", streamer_data.current_tags.join(", ")));
        }

        if !streamer_data.top_clips.is_empty() {
            prompt.push_str("- Has some popular clips\n");
        }

        prompt.push_str(&format!("- Twitch URL: https://twitch.tv/{}\n", user.username));
        prompt.push_str("\nThe shoutout should be enthusiastic, brief (1-2 sentences), and encourage viewers to check out the streamer's channel. Use the streamer's display name (@{}) in the message. Don't directly list all the information, but use it to craft a compelling message. Make sure to include the correct Twitch URL at the end of the message, with a space before and after the URL.");

        // Use the AI client to generate the shoutout message
        let mut shoutout_message = ai_client.generate_response_without_history(&prompt).await?;

        // Ensure the correct URL is in the message with proper spacing
        if !shoutout_message.contains(&format!(" https://twitch.tv/{} ", user.username)) {
            shoutout_message = shoutout_message.trim().to_string();
            shoutout_message.push_str(&format!(" Check them out at https://twitch.tv/{} !", user.username));
        }

        Ok(shoutout_message)
    } else {
        Ok(format!("Go check out @{}! They're an awesome streamer you don't want to miss! Follow them at https://twitch.tv/{}", user.display_name, user.username))
    }
}

async fn generate_simple_shoutout_message(
    twitch_manager: &Arc<TwitchManager>,
    user_id: &str
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let user = twitch_manager.get_user(user_id).await?;

    if let Some(streamer_data) = &user.streamer_data {
        let mut message = format!("Go check out @{}! ", user.display_name);

        if !streamer_data.recent_games.is_empty() {
            message.push_str(&format!("They were last seen playing {}. ", streamer_data.recent_games[0]));
        }

        if !streamer_data.current_title.is_empty() {
            message.push_str(&format!("Their last stream title was: \"{}\". ", streamer_data.current_title));
        }

        message.push_str(&format!("Follow them at https://twitch.tv/{} ", user.username));

        Ok(message)
    } else {
        Ok(format!("Go check out @{}! They're an awesome streamer you don't want to miss! Follow them at https://twitch.tv/{} ", user.display_name, user.username))
    }
}
