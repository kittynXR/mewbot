use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use log::{error, info};
use tokio::time::Duration;
use tokio::sync::Mutex;
use std::collections::HashMap;
use chrono::{DateTime, Utc};

const MAX_IRC_MESSAGE_LENGTH: usize = 500;

pub struct WorldCommand;

#[async_trait::async_trait]
impl Command for WorldCommand {
    fn name(&self) -> &'static str {
        "!world"
    }

    fn description(&self) -> &'static str {
        "Provides information about the current VRChat world"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        lazy_static::lazy_static! {
            static ref COMMAND_COOLDOWNS: Mutex<HashMap<String, DateTime<Utc>>> = Mutex::new(HashMap::new());
        }

        // Add this check at the start of execute():
        let mut cooldowns = COMMAND_COOLDOWNS.lock().await;
        let now = Utc::now();
        if let Some(last_use) = cooldowns.get(&ctx.channel) {
            if (now - *last_use).num_seconds() < 10 {
                return Ok(());
            }
        }
        cooldowns.insert(ctx.channel.clone(), now);

        if !ctx.vrchat_manager.is_online().await {
            info!("VRChat status is offline");
            ctx.bot_client.send_message(&ctx.channel, "The VRChat status is currently offline.").await?;
            return Ok(());
        }

        match ctx.vrchat_manager.get_current_world().await {
            Ok(world) => {
                info!("Successfully fetched current world data");

                // Prepare messages with length checking
                let first_message = format!(
                    "Current World: {} | Author: {} | Capacity: {} | Description: {} | Status: {}",
                    world.name, world.author_name, world.capacity, world.description, world.release_status
                );

                let world_link = format!("https://vrchat.com/home/world/{}", world.id);
                let second_message = format!(
                    "Published: {} | Last Updated: {} | World Link: {}",
                    world.created_at.format("%Y-%m-%d"),
                    world.updated_at.format("%Y-%m-%d"),
                    world_link
                );

                // Split messages if they're too long
                let first_messages = split_message(&first_message);
                let second_messages = split_message(&second_message);

                // Send all parts with proper delays and error handling
                for msg in first_messages {
                    match ctx.bot_client.send_message(&ctx.channel, &msg).await {
                        Ok(_) => {
                            // Significant delay between messages to avoid rate limiting
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        },
                        Err(e) => {
                            error!("Failed to send first part of world info: {:?}", e);
                            // Try to send error message to chat
                            let _ = ctx.bot_client.send_message(
                                &ctx.channel,
                                "Error sending world info. Please try again."
                            ).await;
                            return Err(e);
                        }
                    }
                }

                // Additional delay between message groups
                tokio::time::sleep(Duration::from_secs(1)).await;

                for msg in second_messages {
                    match ctx.bot_client.send_message(&ctx.channel, &msg).await {
                        Ok(_) => {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        },
                        Err(e) => {
                            error!("Failed to send second part of world info: {:?}", e);
                            // Try to send error message to chat
                            let _ = ctx.bot_client.send_message(
                                &ctx.channel,
                                "Error sending world link info. Please try again."
                            ).await;
                            return Err(e);
                        }
                    }
                }
            },
            Err(e) => {
                error!("Error fetching current world information: {:?}", e);
                ctx.bot_client.send_message(
                    &ctx.channel,
                    &format!("Unable to fetch current world information: {}", e)
                ).await?;
            }
        }

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Subscriber
    }
}

fn split_message(message: &str) -> Vec<String> {
    if message.len() <= MAX_IRC_MESSAGE_LENGTH {
        return vec![message.to_string()];
    }

    let mut messages = Vec::new();
    let mut current_message = String::new();
    let words = message.split_whitespace();

    for word in words {
        if current_message.len() + word.len() + 1 > MAX_IRC_MESSAGE_LENGTH {
            if !current_message.is_empty() {
                messages.push(current_message.trim().to_string());
                current_message = String::new();
            }
            // If a single word is too long, split it
            if word.len() > MAX_IRC_MESSAGE_LENGTH {
                for chunk in word.as_bytes().chunks(MAX_IRC_MESSAGE_LENGTH - 3) {
                    let chunk_str = String::from_utf8_lossy(chunk);
                    messages.push(format!("{}...", chunk_str));
                }
            } else {
                current_message = word.to_string();
            }
        } else {
            if !current_message.is_empty() {
                current_message.push(' ');
            }
            current_message.push_str(word);
        }
    }

    if !current_message.is_empty() {
        messages.push(current_message.trim().to_string());
    }

    messages
}