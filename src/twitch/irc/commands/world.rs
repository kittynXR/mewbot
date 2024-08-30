use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use crate::vrchat::models::World;
use log::{error, info};
use std::sync::Arc;
use tokio::time::Duration;

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
        // if !ctx.is_stream_online {
        //     ctx.bot_client.send_message(&ctx.channel, "The world status is not available while the stream is offline.").await?;
        //     return Ok(());
        // }

        if !ctx.vrchat_manager.is_online().await {
            info!("VRChat status is offline");
            ctx.bot_client.send_message(&ctx.channel, "The VRChat status is currently offline.").await?;
            return Ok(());
        }

        match ctx.vrchat_manager.get_current_world().await {
            Ok(world) => {
                info!("Successfully fetched current world data");

                // Prepare both messages
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

                // Send both messages in quick succession
                ctx.bot_client.send_message(&ctx.channel, &first_message).await?;
                tokio::time::sleep(Duration::from_millis(100)).await; // Small delay to ensure order
                ctx.bot_client.send_message(&ctx.channel, &second_message).await?;
            },
            Err(e) => {
                error!("Error fetching current world information: {:?}", e);
                ctx.bot_client.send_message(&ctx.channel, &format!("Unable to fetch current world information: {}", e)).await?;
            }
        }

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Subscriber
    }
}