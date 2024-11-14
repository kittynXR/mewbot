// src/discord/client.rs

use serenity::prelude::*;
use crate::config::Config;
// use crate::storage::StorageClient;
use crate::discord::UserLinks;
use std::sync::Arc;
use std::time::Duration;
use log::{info, warn};
use serenity::http::Http;
use tokio::sync::{RwLock, Mutex};

use super::events::EventHandler;

pub struct DiscordClient {
    client: Arc<Mutex<Option<Client>>>,
}

impl DiscordClient {
    pub async fn new(
        config: Arc<RwLock<Config>>,
        // storage: Arc<RwLock<StorageClient>>,
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
            .event_handler(EventHandler::new(config.clone(), user_links.clone()))
            .await?;

        Ok(Self {
            client: Arc::new(Mutex::new(Some(client))),
        })
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Shutting down DiscordClient...");
        let mut client_guard = self.client.lock().await;
        if let Some(client) = client_guard.take() {
            let shard_manager = client.shard_manager.clone();
            match tokio::time::timeout(Duration::from_secs(10), shard_manager.shutdown_all()).await {
                Ok(_) => info!("Discord shards shut down successfully"),
                Err(_) => warn!("Timed out while shutting down Discord shards"),
            }
        }
        info!("DiscordClient shutdown complete.");
        Ok(())
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

    pub async fn get_http(&self) -> Arc<Http> {
        self.client.lock().await
            .as_ref()
            .expect("Discord client not initialized")
            .http.clone()
    }
}