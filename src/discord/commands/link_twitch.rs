// src/discord/commands/link_twitch.rs

use serenity::builder::{CreateCommand, CreateInteractionResponse};
use serenity::model::prelude::*;
use serenity::prelude::*;
use serenity::builder::CreateInteractionResponseMessage;
use rand::Rng;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use std::sync::Arc;
use log::{error, info};

pub fn register() -> CreateCommand {
    CreateCommand::new("linktwitch")
        .description("Link your Twitch account to Discord")
}

pub async fn run(ctx: Context, command: CommandInteraction, user_links: Arc<UserLinks>) -> Result<(), serenity::Error> {
    let discord_id = command.user.id;

    // Generate a random 6-digit code
    let verification_code: u32 = rand::thread_rng().gen_range(100000..999999);

    // Store the pending verification
    if let Err(e) = user_links.add_pending_verification(discord_id, verification_code).await {
        error!("Error adding pending verification: {:?}", e);
        command.create_response(&ctx.http, CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("An error occurred while processing your request. Please try again later.")
                .ephemeral(true)
        )).await?;
        return Ok(());
    }

    // Send a DM to the user with the verification code
    let dm_channel = discord_id.create_dm_channel(&ctx.http).await?;
    dm_channel.say(&ctx.http, format!(
        "Your Twitch verification code is: {}. Use this command in Twitch chat to verify and link your account: !verify {}",
        verification_code, verification_code
    )).await?;

    // Respond to the slash command
    command.create_response(&ctx.http, CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content("I've sent you a DM with your verification code. Please check your Discord messages and use the code in Twitch chat to complete the linking process.")
            .ephemeral(true)
    )).await?;

    info!("Added pending verification for Discord user: {}", discord_id);

    Ok(())
}