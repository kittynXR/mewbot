use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::vrchat::models::World;

use tokio::sync::RwLock;
use crate::config::Config;

pub struct DiscordClient {
    // Add necessary fields
}

impl DiscordClient {
    pub async fn new(_config: Arc<RwLock<Config>>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        // Implement initialization logic
        todo!()
    }
}

pub struct DiscordHandler {
    world_info: Arc<Mutex<Option<World>>>,
}

#[async_trait]
impl EventHandler for DiscordHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!world" {
            let world_info = self.world_info.lock().await;
            let response = match &*world_info {
                Some(world) => format!(
                    "World Name: {} | Author: {} | Capacity: {} | Description: {} | Status: {} | ID: {}",
                    world.name, world.author_name, world.capacity, world.description, world.release_status, world.id
                ),
                None => "No world information available yet.".to_string(),
            };
            if let Err(why) = msg.channel_id.say(&ctx.http, response).await {
                println!("Error sending message: {:?}", why);
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

pub async fn run_discord_bot(token: &str, world_info: Arc<Mutex<Option<World>>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let handler = DiscordHandler { world_info };
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(handler)
        .await?;

    client.start().await?;

    Ok(())
}