use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use crate::twitch::utils::get_stream_uptime;
use log::{error, warn};

pub struct UptimeCommand;

#[async_trait::async_trait]
impl Command for UptimeCommand {
    fn name(&self) -> &'static str {
        "!uptime"
    }

    fn description(&self) -> &'static str {
        "Shows how long the stream has been live"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        warn!("Starting handle_uptime for channel: {}", ctx.channel);

        match get_stream_uptime(&ctx.channel, ctx.twitch_manager.get_api_client()).await {
            Ok(uptime) => {
                let response = match uptime {
                    Some(duration) => format!(
                        "Stream has been live for {} hours, {} minutes, and {} seconds",
                        duration.num_hours(),
                        duration.num_minutes() % 60,
                        duration.num_seconds() % 60,
                    ),
                    None => "Stream is currently offline.".to_string(),
                };
                ctx.bot_client.send_message(&ctx.channel, &response).await?;
            },
            Err(e) => {
                error!("Error getting stream uptime: {:?}", e);
                let error_response = "Sorry, I couldn't retrieve the stream uptime. Please try again later.".to_string();
                ctx.bot_client.send_message(&ctx.channel, &error_response).await?;
            }
        }
        warn!("Completed handle_uptime");
        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Viewer
    }
}