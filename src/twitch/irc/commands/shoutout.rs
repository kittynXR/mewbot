use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use crate::ai::AIClient;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use log::{error};
use crate::twitch::TwitchManager;
use crate::twitch::models::shoutout::{GLOBAL_COOLDOWN_SECONDS, USER_COOLDOWN_SECONDS};



pub struct ShoutoutQueueItem {
    pub(crate) user_id: String,
    pub(crate) username: String,
    enqueue_time: Instant,
}

pub struct ShoutoutCooldown {
    last_global_shoutout: Instant,
    per_user: HashMap<String, Instant>,
    queue: VecDeque<ShoutoutQueueItem>,
}

impl Default for ShoutoutCooldown {
    fn default() -> Self {
        Self::new()
    }
}

impl ShoutoutCooldown {
    pub fn new() -> Self {
        Self {
            last_global_shoutout: Instant::now() - Duration::from_secs(GLOBAL_COOLDOWN_SECONDS + 1),
            per_user: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn can_shoutout(&self) -> bool {
        Instant::now().duration_since(self.last_global_shoutout) >= Duration::from_secs(GLOBAL_COOLDOWN_SECONDS)
    }

    pub fn update_cooldowns(&mut self, user_id: &str) {
        let now = Instant::now();
        self.last_global_shoutout = now;
        self.per_user.insert(user_id.to_string(), now);
    }

    pub fn enqueue(&mut self, user_id: String, username: String) {
        self.queue.push_back(ShoutoutQueueItem {
            user_id,
            username,
            enqueue_time: Instant::now(),
        });
    }

    pub fn dequeue(&mut self) -> Option<ShoutoutQueueItem> {
        self.queue.pop_front()
    }

    pub fn requeue(&mut self, item: ShoutoutQueueItem) {
        self.queue.push_front(item);
    }

    pub fn remove(&mut self, user_id: &str) {
        self.queue.retain(|item| item.user_id != user_id);
    }
}

pub struct ShoutoutCommand;

#[async_trait::async_trait]
impl Command for ShoutoutCommand {
    fn name(&self) -> &'static str {
        "!so"
    }

    fn description(&self) -> &'static str {
        "Gives a shoutout to another streamer"
    }

    fn required_role(&self) -> UserRole {
        UserRole::Subscriber
    }

    async fn execute(&self, ctx: &CommandContext, args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if args.is_empty() {
            ctx.bot_client.send_message(&ctx.channel, "Usage: !so <username>").await?;
            return Ok(());
        }

        let target_username = args[0].trim_start_matches('@').to_lowercase();

        // Check if the user is trying to shout themselves out
        if target_username.to_lowercase() == ctx.msg.sender.name.to_lowercase() {
            let self_shoutout_message = format!("@{}, you're already awesome! No need to shout yourself out! pepeStepBro pepeStepBro ", ctx.msg.sender.name);
            ctx.bot_client.send_message(&ctx.channel, &self_shoutout_message).await?;
            return Ok(());
        }

        // Check if the target is the broadcaster
        if target_username.to_lowercase() == ctx.channel.to_lowercase() {
            let broadcaster_shoutout_message = format!("@{}, 推し～！ Cannot shout out our oshi {}! They're already here blessing us with their sugoi presence! ٩(◕‿◕｡)۶", ctx.msg.sender.name, ctx.channel);
            ctx.bot_client.send_message(&ctx.channel, &broadcaster_shoutout_message).await?;
            return Ok(());
        }

        // Generate and send shoutout message
        let shoutout_message = generate_shoutout_message(&ctx.twitch_manager, &ctx.ai_client, &target_username).await?;

        // Send the shoutout message
        ctx.bot_client.send_message(&ctx.channel, &shoutout_message).await?;

        // Queue the API shoutout
        let api_client = ctx.twitch_manager.get_api_client();
        match api_client.get_user_info(&target_username).await {
            Ok(user_info) => {
                if let Some(user) = user_info["data"].as_array().and_then(|arr| arr.first()) {
                    let user_id = user["id"].as_str().unwrap_or("");
                    let display_name = user["display_name"].as_str().unwrap_or(&target_username);
                    ctx.twitch_manager.queue_shoutout(user_id.to_string(), display_name.to_string()).await;
                }
            },
            Err(e) => {
                error!("Error fetching user info: {:?}", e);
            }
        }

        Ok(())
    }
}

async fn generate_shoutout_message(
    twitch_manager: &Arc<TwitchManager>,
    ai_client: &Option<Arc<AIClient>>,
    target_username: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let api_client = twitch_manager.get_api_client();
    let user_info = api_client.get_user_info(target_username).await?;
    let user_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get user ID")?.to_string();

    if let Err(e) = twitch_manager.update_streamer_data(&user_id).await {
        error!("Failed to update streamer data: {}", e);
    }

    match ai_client {
        Some(ai) => {
            match generate_ai_shoutout_message(twitch_manager, ai, &user_id).await {
                Ok(message) => Ok(message),
                Err(e) => {
                    error!("Failed to generate AI shoutout message: {:?}", e);
                    generate_simple_shoutout_message(twitch_manager, &user_id).await
                }
            }
        },
        None => generate_simple_shoutout_message(twitch_manager, &user_id).await,
    }
}

async fn generate_ai_shoutout_message(
    twitch_manager: &Arc<TwitchManager>,
    ai_client: &Arc<AIClient>,
    user_id: &str
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let mut user = twitch_manager.get_user(user_id).await?;

    // Double-check user information
    if user.display_name == "displayname" || user.username == user_id {
        twitch_manager.update_streamer_data(user_id).await?;
        let updated_user = twitch_manager.get_user(user_id).await?;
        if updated_user.display_name != "displayname" && updated_user.username != user_id {
            user = updated_user;
        } else {
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

        if !streamer_data.recent_titles.is_empty() {
            prompt.push_str("- Recent stream titles:\n");
            for (i, title) in streamer_data.recent_titles.iter().enumerate().take(3) {
                prompt.push_str(&format!("  {}. \"{}\"\n", i + 1, title));
            }
        }

        if !streamer_data.current_tags.is_empty() {
            prompt.push_str(&format!("- Stream tags: {}\n", streamer_data.current_tags.join(", ")));
        }

        if !streamer_data.top_clips.is_empty() {
            prompt.push_str("- Popular clips:\n");
            for (i, (title, _)) in streamer_data.top_clips.iter().enumerate().take(3) {
                prompt.push_str(&format!("  {}. \"{}\"\n", i + 1, title));
            }
        }

        prompt.push_str(&format!("- Twitch URL: https://twitch.tv/{}\n", user.username));
        prompt.push_str("\nThe shoutout should be enthusiastic, brief (1-2 sentences), and encourage viewers to check out the streamer's channel. Use the streamer's display name (@{}) in the message. Don't directly list all the information, but use it to craft a compelling message. If relevant, mention a recent game, stream title, or popular clip. Make sure to include the correct Twitch URL at the end of the message, with a space before and after the URL.");

        // Use the AI client to generate the shoutout message
        let mut shoutout_message = ai_client.generate_response_without_history(&prompt).await?;

        // Ensure the correct URL is in the message with proper spacing
        if !shoutout_message.contains(&format!(" https://twitch.tv/{} ", user.username)) {
            shoutout_message = shoutout_message.trim().to_string();
            shoutout_message.push_str(&format!(" Check them out at https://twitch.tv/{} !", user.username));
        }

        Ok(shoutout_message)
    } else {
        generate_simple_shoutout_message(twitch_manager, user_id).await
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