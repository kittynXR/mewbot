use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use crate::twitch::api::requests::announcement::send_announcement;
use crate::ai::AIClient;
use std::sync::Arc;

pub struct VRCCommand;

#[async_trait::async_trait]
impl Command for VRCCommand {
    fn name(&self) -> &'static str {
        "!vrc"
    }

    fn description(&self) -> &'static str {
        "Provides a link to join our VRChat community group and sends an announcement"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let irc_manager = ctx.bot_client.get_manager();
        let vrc_link = irc_manager.get_vrchat_group_link().await;

        // Generate a custom greeting using AI
        let vrc_message = generate_vrc_message(&ctx.ai_client, &vrc_link).await;

        let api_client = ctx.twitch_manager.get_api_client();
        // Send an announcement
        let broadcaster_id = api_client.get_broadcaster_id().await?;

        send_announcement(api_client, &broadcaster_id, &broadcaster_id, &vrc_message, Some("primary")).await?;

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Viewer
    }
}

async fn generate_vrc_message(ai_client: &Option<Arc<AIClient>>, vrc_link: &str) -> String {
    if let Some(ai) = ai_client {
        let prompt = format!(
            "Generate a friendly and inviting message to encourage Twitch viewers to join our VRChat community group. \
            Feel free to mention: VR, technology, cute & funny anime, catgirls, foxgirls, \
            catboys, foxboys, 3D art or living in the matrix. \
            Don't use the word viewers.  If anything, say chatters or everyone. Good vibes. Good vibes. \
            The message should be brief (1-2 sentences) and include the following VRChat group link: {}. \
            Make sure the tone is casual and welcoming.",
            vrc_link
        );

        match ai.generate_response_without_history(&prompt).await {
            Ok(response) => {
                // Add spaces around the vrc_link within the response
                let cleaned_response = response.replace(vrc_link, &format!(" {} ", vrc_link));

                // Trim any leading or trailing whitespace
                cleaned_response.trim().to_string()
            }
            Err(e) => {
                eprintln!("Error generating AI response: {:?}", e);
                default_vrc_message(vrc_link)
            }
        }
    } else {
        default_vrc_message(vrc_link)
    }
}

fn default_vrc_message(vrc_link: &str) -> String {
    format!("Join our VRChat community group! {} ", vrc_link)
}