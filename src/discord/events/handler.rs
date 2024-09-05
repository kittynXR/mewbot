use serenity::async_trait;
use serenity::model::prelude::*;
use serenity::prelude::*;
use crate::config::Config;
// use crate::storage::StorageClient;
use crate::discord::UserLinks;
use std::sync::Arc;
use log::{debug, error, info};
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use tokio::sync::RwLock;
use crate::discord::commands::{link_twitch, ping};

pub struct EventHandler {
    config: Arc<RwLock<Config>>,
    user_links: Arc<UserLinks>,
}

impl EventHandler {
    pub fn new(config: Arc<RwLock<Config>>, user_links: Arc<UserLinks>) -> Self {
        Self { config, user_links }
    }
}

#[async_trait]
impl serenity::client::EventHandler for EventHandler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let guild_id = {
            let config_read = self.config.read().await;
            config_read.discord_guild_id.clone().and_then(|id| id.parse::<u64>().ok())
        };

        if let Some(guild_id) = guild_id {
            let guild_id = GuildId::new(guild_id);
            let commands = guild_id.set_commands(&ctx.http, vec![
                ping::register(),
                link_twitch::register(),
                // Add more slash commands here
            ]).await;

            debug!("Slash commands registered: {:#?}", commands);
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            info!("Received command interaction: {:#?}", command);

            let result = match command.data.name.as_str() {
                "ping" => ping::run(ctx, command).await,
                "linktwitch" => link_twitch::run(ctx, command, self.user_links.clone()).await,
                _ => {
                    command.create_response(&ctx.http, CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().content("Command not implemented")
                    )).await
                }
            };

            if let Err(why) = result {
                error!("Cannot respond to slash command: {}", why);
            }
        }
    }
}