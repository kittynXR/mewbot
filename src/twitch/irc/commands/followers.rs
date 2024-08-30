use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use log::error;
use chrono::{DateTime, Utc};

pub struct FollowersCommand;
pub struct FollowAgeCommand;

#[async_trait::async_trait]
impl Command for FollowersCommand {
    fn name(&self) -> &'static str {
        "!followers"
    }

    fn description(&self) -> &'static str {
        "Shows the current number of followers for the channel"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let twitch_api_client = ctx.twitch_manager.get_api_client();

        match twitch_api_client.get_broadcaster_id().await {
            Ok(broadcaster_id) => {
                match twitch_api_client.get_follower_count(&broadcaster_id).await {
                    Ok(follower_count) => {
                        let response = format!("@{}, the channel currently has {} followers!", ctx.msg.sender.name, follower_count);
                        ctx.bot_client.send_message(&ctx.channel, &response).await?;
                    },
                    Err(e) => {
                        error!("Failed to get follower count: {:?}", e);
                        ctx.bot_client.send_message(&ctx.channel, "Sorry, I couldn't retrieve the follower count at the moment.").await?;
                    }
                }
            },
            Err(e) => {
                error!("Failed to get broadcaster ID: {:?}", e);
                ctx.bot_client.send_message(&ctx.channel, "Sorry, I couldn't retrieve the channel information at the moment.").await?;
            }
        }

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Viewer
    }
}

#[async_trait::async_trait]
impl Command for FollowAgeCommand {
    fn name(&self) -> &'static str {
        "!followage"
    }

    fn description(&self) -> &'static str {
        "Shows how long a user has been following the channel"
    }

    async fn execute(&self, ctx: &CommandContext, args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let twitch_api_client = ctx.twitch_manager.get_api_client();
        let broadcaster_id = twitch_api_client.get_broadcaster_id().await?;

        let target_user = if !args.is_empty() {
            args[0].trim_start_matches('@')
        } else {
            &ctx.msg.sender.name
        };

        match twitch_api_client.get_user_info(target_user).await {
            Ok(user_info) => {
                let user_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get user ID")?;

                match twitch_api_client.get_follower_info(&broadcaster_id, Some(user_id)).await {
                    Ok((followers, _total)) => {
                        if let Some(follower) = followers.first() {
                            let follow_duration = format_duration(follower.followed_at, Utc::now());
                            let response = format!("@{}, {} has been following the channel for {}!",
                                                   ctx.msg.sender.name, target_user, follow_duration);
                            ctx.bot_client.send_message(&ctx.channel, &response).await?;
                        } else {
                            let response = format!("@{}, {} is not following this channel.", ctx.msg.sender.name, target_user);
                            ctx.bot_client.send_message(&ctx.channel, &response).await?;
                        }
                    },
                    Err(e) => {
                        error!("Failed to get follower info: {:?}", e);
                        ctx.bot_client.send_message(&ctx.channel, &format!("Sorry, I couldn't retrieve follow information for {}.", target_user)).await?;
                    }
                }
            },
            Err(e) => {
                error!("Failed to get user info: {:?}", e);
                ctx.bot_client.send_message(&ctx.channel, &format!("Sorry, I couldn't retrieve information for user {}.", target_user)).await?;
            }
        }

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Viewer
    }
}

fn format_duration(start: DateTime<Utc>, end: DateTime<Utc>) -> String {
    let duration = end.signed_duration_since(start);
    let years = duration.num_days() / 365;
    let months = (duration.num_days() % 365) / 30;
    let days = duration.num_days() % 30;
    let hours = duration.num_hours() % 24;
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;

    let mut parts = Vec::new();

    if years > 0 {
        parts.push(format!("{} year{}", years, if years > 1 { "s" } else { "" }));
    }
    if months > 0 {
        parts.push(format!("{} month{}", months, if months > 1 { "s" } else { "" }));
    }
    if days > 0 {
        parts.push(format!("{} day{}", days, if days > 1 { "s" } else { "" }));
    }
    if hours > 0 {
        parts.push(format!("{} hour{}", hours, if hours > 1 { "s" } else { "" }));
    }
    if minutes > 0 {
        parts.push(format!("{} minute{}", minutes, if minutes > 1 { "s" } else { "" }));
    }
    if seconds > 0 || parts.is_empty() {
        parts.push(format!("{} second{}", seconds, if seconds != 1 { "s" } else { "" }));
    }

    match parts.len() {
        0 => "0 seconds".to_string(),
        1 => parts[0].clone(),
        2 => format!("{} and {}", parts[0], parts[1]),
        _ => {
            let last = parts.pop().unwrap();
            format!("{}, and {}", parts.join(", "), last)
        }
    }
}