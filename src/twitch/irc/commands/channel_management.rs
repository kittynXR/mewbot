use std::error::Error;
use rand::Rng;
use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use crate::twitch::api::requests::channel;

pub struct TitleCommand;
pub struct GameCommand;
pub struct ContentCommand;
pub struct RunAdCommand;

pub struct RefreshAdsCommand;

#[async_trait::async_trait]
impl Command for RefreshAdsCommand {
    fn name(&self) -> &'static str {
        "!refreshads"
    }

    fn description(&self) -> &'static str {
        "Refreshes ad status with content label change and commercial"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn Error + Send + Sync>> {
        let api_client = ctx.twitch_manager.get_api_client();
        let broadcaster_id = match api_client.get_broadcaster_id().await {
            Ok(id) => id,
            Err(err) => {
                ctx.bot_client.send_message(&ctx.channel, &format!("Failed to get broadcaster ID: {}", err)).await?;
                return Ok(()); // Exit early if broadcaster ID cannot be fetched
            }
        };

        // 1. Set the content label
        if let Err(err) = channel::update_content_classification_labels(
            &api_client,
            &broadcaster_id,
            vec![("DebatedSocialIssuesAndPolitics".to_string(), true)]
        ).await {
            ctx.bot_client.send_message(&ctx.channel, &format!("Failed to set content label: {}", err)).await?;
        } else {
            // ctx.bot_client.send_message(&ctx.channel, "Step 1/4: Content label set successfully.").await?;
        }

        let wait_time = rand::thread_rng().gen_range(10..=20);
        // ctx.bot_client.send_message(&ctx.channel, &format!("Step 2/4: Waiting for {} seconds...", wait_time)).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(wait_time)).await;

        // 3. Run the ad
        if let Err(err) = channel::start_commercial(&api_client, &broadcaster_id, 180).await {
            ctx.bot_client.send_message(&ctx.channel, &format!("Failed to start the commercial: {}", err)).await?;
        } else {
            // ctx.bot_client.send_message(&ctx.channel, "Step 3/4: Commercial started successfully.").await?;
        }

        // 4. Remove the content label
        if let Err(err) = channel::update_content_classification_labels(
            &api_client,
            &broadcaster_id,
            vec![("DebatedSocialIssuesAndPolitics".to_string(), false)]
        ).await {
            ctx.bot_client.send_message(&ctx.channel, &format!("Failed to remove content label: {}", err)).await?;
        } else {
            // ctx.bot_client.send_message(&ctx.channel, "Step 4/4: Content label removed successfully.").await?;
        }

        ctx.bot_client.send_message(&ctx.channel, "Ad refresh complete! wanwan wanwan").await?;
        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Moderator
    }
}

#[async_trait::async_trait]
impl Command for TitleCommand {
    fn name(&self) -> &'static str {
        "!title"
    }

    fn description(&self) -> &'static str {
        "Sets the stream title"
    }

    async fn execute(&self, ctx: &CommandContext, args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if args.is_empty() {
            ctx.bot_client.send_message(&ctx.channel, "Usage: !title <new title>").await?;
            return Ok(());
        }

        let new_title = args.join(" ");
        let api_client = ctx.twitch_manager.get_api_client();
        let broadcaster_id = api_client.get_broadcaster_id().await?;

        channel::update_channel_title(&api_client, &broadcaster_id, &new_title).await?;

        ctx.bot_client.send_message(&ctx.channel, &format!("Stream title updated to: {}", new_title)).await?;
        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Moderator
    }
}

#[async_trait::async_trait]
impl Command for GameCommand {
    fn name(&self) -> &'static str {
        "!game"
    }

    fn description(&self) -> &'static str {
        "Sets the stream category/game"
    }

    async fn execute(&self, ctx: &CommandContext, args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if args.is_empty() {
            ctx.bot_client.send_message(&ctx.channel, "Usage: !game <category name>").await?;
            return Ok(());
        }

        let game_name = args.join(" ");
        let api_client = ctx.twitch_manager.get_api_client();
        let broadcaster_id = api_client.get_broadcaster_id().await?;

        // First search for the game
        match channel::search_category(&api_client, &game_name).await? {
            Some((game_id, exact_name)) => {
                // Update the category with the found game ID
                channel::update_channel_category(&api_client, &broadcaster_id, &game_id).await?;
                ctx.bot_client.send_message(&ctx.channel,
                                            &format!("Stream category updated to: {}", exact_name)
                ).await?;
            },
            None => {
                ctx.bot_client.send_message(&ctx.channel,
                                            &format!("Could not find a category matching '{}'", game_name)
                ).await?;
            }
        }

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Moderator
    }
}

#[async_trait::async_trait]
impl Command for ContentCommand {
    fn name(&self) -> &'static str {
        "!content"
    }

    fn description(&self) -> &'static str {
        "Sets content classification labels"
    }

    async fn execute(&self, ctx: &CommandContext, args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if args.is_empty() {
            ctx.bot_client.send_message(&ctx.channel,
                                        "Usage: !content <label> <on/off> - Available labels: DebatedSocialIssuesAndPolitics, DrugsIntoxication, SexualThemes, ViolentGraphic, Gambling, ProfanityVulgarity"
            ).await?;
            return Ok(());
        }

        let label = args[0].to_string();
        let enable = args.get(1).map(|s| s.to_lowercase() == "on").unwrap_or(true);

        let api_client = ctx.twitch_manager.get_api_client();
        let broadcaster_id = api_client.get_broadcaster_id().await?;

        let labels = vec![(label.clone(), enable)];
        channel::update_content_classification_labels(&api_client, &broadcaster_id, labels).await?;

        ctx.bot_client.send_message(&ctx.channel,
                                    &format!("Content label {} has been turned {}", label, if enable { "on" } else { "off" })
        ).await?;
        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Moderator
    }
}

#[async_trait::async_trait]
impl Command for RunAdCommand {
    fn name(&self) -> &'static str {
        "!runad"
    }

    fn description(&self) -> &'static str {
        "Runs a 3-minute ad"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let api_client = ctx.twitch_manager.get_api_client();
        let broadcaster_id = api_client.get_broadcaster_id().await?;

        channel::start_commercial(&api_client, &broadcaster_id, 180).await?;

        ctx.bot_client.send_message(&ctx.channel, "Starting a 3-minute ad break...").await?;
        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Moderator
    }
}