use std::error::Error;
use tokio::sync::Mutex;
use rand::Rng;
use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use crate::twitch::api::requests::channel;

// Add these imports:
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::irc::TwitchBotClient;

pub struct TitleCommand;
pub struct GameCommand;
pub struct ContentCommand;
pub struct RunAdCommand;
pub struct RefreshAdsCommand;
pub struct AdNomsterCommand;

lazy_static::lazy_static! {
    static ref AD_NOMSTER_TASK: Mutex<Option<tokio::task::JoinHandle<()>>> = Mutex::new(None);
}

#[async_trait::async_trait]
impl Command for RefreshAdsCommand {
    fn name(&self) -> &'static str {
        "!refreshads"
    }

    fn description(&self) -> &'static str {
        "Refreshes ad status with content label change and commercial"
    }


    async fn execute(
        &self,
        ctx: &CommandContext,
        _args: Vec<String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let api_client = ctx.twitch_manager.get_api_client();
        refresh_ads(&api_client, &ctx.bot_client, &ctx.channel).await
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

#[async_trait::async_trait]
impl Command for AdNomsterCommand {
    fn name(&self) -> &'static str {
        "!adnomster"
    }

    fn description(&self) -> &'static str {
        "Starts or stops the automatic ad runner every 59 minutes"
    }

    async fn execute(
        &self,
        ctx: &CommandContext,
        args: Vec<String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut task_handle = AD_NOMSTER_TASK.lock().await;

        match args.get(0).map(|s| s.to_lowercase()) {
            Some(ref cmd) if cmd == "start" => {
                // Start the background task
                if task_handle.is_some() {
                    ctx.bot_client
                        .send_message(&ctx.channel, "Ad Nomster is already running.")
                        .await?;
                } else {
                    let bot_client = ctx.bot_client.clone();
                    let channel = ctx.channel.clone();
                    let twitch_manager = ctx.twitch_manager.clone();

                    let handle = tokio::spawn(async move {
                        let api_client = twitch_manager.get_api_client();
                        loop {
                            if let Err(e) = refresh_ads(&api_client, &bot_client, &channel).await {
                                eprintln!("Error executing refresh_ads: {:?}", e);
                            }
                            // Wait for 59 minutes
                            tokio::time::sleep(tokio::time::Duration::from_secs(59 * 60)).await;
                        }
                    });

                    *task_handle = Some(handle);
                    ctx.bot_client
                        .send_message(&ctx.channel, "Ad Nomster has been started.")
                        .await?;
                }
            }
            Some(ref cmd) if cmd == "stop" => {
                // Stop the background task if it's running
                if let Some(handle) = task_handle.take() {
                    handle.abort();
                    ctx.bot_client
                        .send_message(&ctx.channel, "Ad Nomster has been stopped.")
                        .await?;
                } else {
                    ctx.bot_client
                        .send_message(&ctx.channel, "Ad Nomster is not running.")
                        .await?;
                }
            }
            _ => {
                // Provide help text
                ctx.bot_client
                    .send_message(
                        &ctx.channel,
                        "Usage: !adnomster <start|stop> - Starts or stops the automatic ad runner every 59 minutes.",
                    )
                    .await?;
            }
        }

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::VIP
    }
}

pub async fn refresh_ads(
    api_client: &TwitchAPIClient,
    bot_client: &TwitchBotClient,
    channel: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let broadcaster_id = match api_client.get_broadcaster_id().await {
        Ok(id) => id,
        Err(err) => {
            bot_client
                .send_message(channel, &format!("Failed to get broadcaster ID: {}", err))
                .await?;
            return Ok(()); // Exit early if broadcaster ID cannot be fetched
        }
    };

    // 1. Set the content label
    if let Err(err) = channel::update_content_classification_labels(
        api_client,
        &broadcaster_id,
        vec![("DebatedSocialIssuesAndPolitics".to_string(), true)],
    )
        .await
    {
        bot_client
            .send_message(channel, &format!("Failed to set content label: {}", err))
            .await?;
    }

    let wait_time = rand::thread_rng().gen_range(10..=20);
    tokio::time::sleep(tokio::time::Duration::from_secs(wait_time)).await;

    // 3. Run the ad
    if let Err(err) = channel::start_commercial(api_client, &broadcaster_id, 180).await {
        bot_client
            .send_message(channel, &format!("Failed to start the commercial: {}", err))
            .await?;
    }

    // 4. Remove the content label
    if let Err(err) = channel::update_content_classification_labels(
        api_client,
        &broadcaster_id,
        vec![("DebatedSocialIssuesAndPolitics".to_string(), false)],
    )
        .await
    {
        bot_client
            .send_message(channel, &format!("Failed to remove content label: {}", err))
            .await?;
    }

    bot_client
        .send_message(channel, "Ad refresh complete! wanwan wanwan")
        .await?;
    Ok(())
}