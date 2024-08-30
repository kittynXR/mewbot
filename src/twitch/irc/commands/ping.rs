use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;

pub struct PingCommand;

#[async_trait::async_trait]
impl Command for PingCommand {
    fn name(&self) -> &'static str {
        "!ping"
    }

    fn description(&self) -> &'static str {
        "Responds with Pong!"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        ctx.bot_client.send_message(&ctx.channel, "Pong!").await?;
        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Viewer
    }
}