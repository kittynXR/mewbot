use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use log::{info, warn};

pub struct VerifyCommand;

#[async_trait::async_trait]
impl Command for VerifyCommand {
    fn name(&self) -> &'static str {
        "!verify"
    }

    fn description(&self) -> &'static str {
        "Verifies and links your Twitch account to your Discord account"
    }

    async fn execute(&self, ctx: &CommandContext, args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if args.len() != 1 {
            ctx.bot_client.send_message(&ctx.channel, "Usage: !verify <code>").await?;
            return Ok(());
        }

        let code = args[0].parse::<u32>().map_err(|_| "Invalid verification code")?;
        let twitch_username = ctx.msg.sender.name.clone();

        info!("Attempting to verify Twitch user: {} with code: {}", twitch_username, code);

        match ctx.user_links.verify_and_link(&twitch_username, code).await {
            Ok(discord_id) => {
                info!("Successfully linked Twitch user: {} to Discord ID: {}", twitch_username, discord_id);
                ctx.bot_client.send_message(&ctx.channel, &format!("@{}, your Twitch account has been successfully verified and linked to your Discord account!", twitch_username)).await?;
            },
            Err(e) => {
                warn!("Verification failed for Twitch user: {}. Error: {}", twitch_username, e);
                ctx.bot_client.send_message(&ctx.channel, &format!("@{}, verification failed: {}. Please use the /linktwitch command in Discord to get a new verification code.", twitch_username, e)).await?;
            }
        }

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Viewer
    }
}