use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;

pub struct ContinueCommand;

#[async_trait::async_trait]
impl Command for ContinueCommand {
    fn name(&self) -> &'static str {
        "!continue"
    }

    fn description(&self) -> &'static str {
        "Continues a previously truncated AI response"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ai_client) = &ctx.ai_client {
            if let Some(remainder) = ai_client.get_remainder(&ctx.msg.sender.id).await {
                if remainder.len() > 500 {
                    let message = format!("{}...", &remainder[..497]);
                    ai_client.store_remainder(ctx.msg.sender.id.clone(), remainder[497..].to_string()).await;
                    ctx.bot_client.send_message(&ctx.channel, &format!("{} (Use !continue to see more)", message)).await?;
                } else {
                    ctx.bot_client.send_message(&ctx.channel, &remainder).await?;
                }
            } else {
                ctx.bot_client.send_message(&ctx.channel, "No continuation available.").await?;
            }
        }
        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Viewer
    }
}