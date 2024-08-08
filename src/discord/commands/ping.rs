// src/discord/commands/ping.rs
use serenity::builder::{CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::model::prelude::*;
use serenity::prelude::*;

pub fn register() -> CreateCommand {
    CreateCommand::new("ping")
        .description("A simple ping command to check if the bot is responsive")
}

pub async fn run(ctx: Context, command: CommandInteraction) -> Result<(), serenity::Error> {
    command.create_response(&ctx.http, CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content("Pong! 🏓")
            .ephemeral(true)
    )).await?;

    Ok(())
}