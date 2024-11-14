use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use log::{error, info};

pub struct ResetDropGameCommand;

#[async_trait::async_trait]
impl Command for ResetDropGameCommand {
    fn name(&self) -> &'static str {
        "!resetdrop"
    }

    fn description(&self) -> &'static str {
        "Resets the Parachute Drop game browser source"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        const BROWSER_SOURCE_NAME: &str = "ParachuteDrop";
        const TARGET_INSTANCE: &str = "Instance2";

        info!("Attempting to reset game on {} in {}", BROWSER_SOURCE_NAME, TARGET_INSTANCE);

        match ctx.obs_manager.refresh_source(TARGET_INSTANCE, BROWSER_SOURCE_NAME).await {
            Ok(_) => {
                info!("Refresh command sent successfully");
                let response = format!(
                    "@{}, sending refresh command to browser source. If the game doesn't reset, please let us know!",
                    ctx.msg.sender.name
                );
                ctx.bot_client.send_message(&ctx.channel, &response).await?;
            },
            Err(e) => {
                error!("Failed to send refresh command: {:?}", e);
                let response = format!(
                    "@{}, failed to reset the game: {}",
                    ctx.msg.sender.name,
                    e
                );
                ctx.bot_client.send_message(&ctx.channel, &response).await?;
            }
        };

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Moderator
    }
}