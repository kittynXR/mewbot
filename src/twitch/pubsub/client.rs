use crate::config::Config;
use crate::twitch::TwitchAPIClient;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub struct PubSubClient {
    config: Arc<RwLock<Config>>,
    api_client: Arc<TwitchAPIClient>,
}

impl PubSubClient {
    pub fn new(config: Arc<RwLock<Config>>, api_client: Arc<TwitchAPIClient>) -> Self {
        Self { config, api_client }
    }

    pub async fn connect_and_listen(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (ws_stream, _) = connect_async("wss://pubsub-edge.twitch.tv").await?;
        println!("WebSocket connected");

        let (mut write, mut read) = ws_stream.split();

        // Authenticate and subscribe
        let token = match self.api_client.get_token().await {
            Ok(t) => t,
            Err(e) => return Err(format!("Failed to get API token: {}", e).into()),
        };

        let channel_id = match self.get_channel_id().await {
            Ok(id) => id,
            Err(e) => return Err(format!("Failed to get channel ID: {}", e).into()),
        };

        let topics = vec![format!("channel-update.{}", channel_id)];
        let message = json!({
        "type": "LISTEN",
        "nonce": "nonce",
        "data": {
            "topics": topics,
            "auth_token": token
        }
    });

        write.send(Message::Text(message.to_string())).await?;
        println!("Subscribed to topics: {:?}", topics);

        // Handle incoming messages
        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    self.handle_message(&text).await?;
                }
                Ok(Message::Close(_)) => {
                    println!("WebSocket closed");
                    break;
                }
                Err(e) => {
                    eprintln!("Error: {:?}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn get_channel_id(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.read().await;
        let channel_name = config.twitch_channel_to_join.clone().ok_or("Channel name not set in config")?;
        drop(config);

        println!("Attempting to get channel ID for: {}", channel_name);

        // Use the Twitch API to get the channel ID
        let user_info = self.api_client.get_user_info(&channel_name).await?;

        println!("Received user info: {:?}", user_info);

        let channel_id = user_info["data"][0]["id"]
            .as_str()
            .ok_or("Channel ID not found in API response")?
            .to_string();

        println!("Retrieved channel ID: {}", channel_id);

        Ok(channel_id)
    }

    async fn handle_message(&self, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let parsed: serde_json::Value = serde_json::from_str(message)?;

        if let Some(data) = parsed.get("data") {
            if let Some(topic) = data.get("topic") {
                if topic.as_str().unwrap_or("").starts_with("channel-update") {
                    crate::twitch::pubsub::handlers::handle_stream_update(data).await?;
                }
            }
        }

        Ok(())
    }
}