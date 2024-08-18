// src/discord/client.rs

use serenity::prelude::*;
use crate::config::Config;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};

use super::events::EventHandler;

pub struct DiscordClient {
    client: Arc<Mutex<Option<Client>>>,
    storage: Arc<RwLock<StorageClient>>,
    user_links: Arc<UserLinks>,
    config: Arc<RwLock<Config>>,
}

impl DiscordClient {
    pub async fn new(
        config: Arc<RwLock<Config>>,
        storage: Arc<RwLock<StorageClient>>,
        user_links: Arc<UserLinks>
    ) -> Result<Self, serenity::Error> {
        let token = {
            let config_read = config.read().await;
            config_read.discord_token.clone().ok_or_else(|| {
                serenity::Error::Other("Discord token not found in configuration")
            })?
        };

        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT
            | GatewayIntents::GUILD_MESSAGE_REACTIONS;

        let client = Client::builder(&token, intents)
            .event_handler(EventHandler::new(config.clone(), storage.clone(), user_links.clone()))
            .await?;

        Ok(Self {
            client: Arc::new(Mutex::new(Some(client))),
            storage,
            user_links,
            config
        })
    }

    pub async fn start(&self) -> Result<(), serenity::Error> {
        let mut client_guard = self.client.lock().await;
        if let Some(mut client) = client_guard.take() {
            client.start().await?;
            *client_guard = Some(client);
            Ok(())
        } else {
            Err(serenity::Error::Other("Discord client has already been started"))
        }
    }
}