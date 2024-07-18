use crate::config::Config;
use crate::twitch::TwitchAPIClient;
use crate::twitch::irc::TwitchIRCClient;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use super::handlers;

use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;


pub struct TwitchEventSubClient {
    config: Arc<RwLock<Config>>,
    api_client: Arc<TwitchAPIClient>,
    irc_client: Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    http_client: Client,
    channel: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct WebSocketResponse {
    metadata: WebSocketMetadata,
    payload: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct WebSocketMetadata {
    message_type: String,
    message_id: String,
}

impl TwitchEventSubClient {
    pub fn new(
        config: Arc<RwLock<Config>>,
        api_client: Arc<TwitchAPIClient>,
        irc_client: Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
        channel: String,
    ) -> Self {
        Self {
            config,
            api_client,
            irc_client,
            http_client: Client::new(),
            channel,
        }
    }

    pub async fn connect_and_listen(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (ws_stream, _) = connect_async("wss://eventsub.wss.twitch.tv/ws").await?;
        println!("EventSub WebSocket connected");

        let (mut write, mut read) = ws_stream.split();

        let mut session_id = String::new();

        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    println!("Received WebSocket message: {}", text);
                    let response: WebSocketResponse = serde_json::from_str(&text)?;
                    if response.metadata.message_type == "session_welcome" {
                        if let Some(session) = response.payload.get("session") {
                            session_id = session["id"].as_str().unwrap_or("").to_string();
                            println!("EventSub session established. Session ID: {}", session_id);
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    println!("EventSub WebSocket closed before session establishment");
                    return Ok(());
                }
                Err(e) => {
                    eprintln!("EventSub WebSocket error: {:?}", e);
                    return Ok(());
                }
                _ => {}
            }
        }

        // Create EventSub subscription
        self.create_eventsub_subscription(&session_id).await?;

        // Handle incoming messages
        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    handlers::handle_message(&text, &self.irc_client, &self.channel).await?;
                }
                Ok(Message::Close(_)) => {
                    println!("EventSub WebSocket closed");
                    break;
                }
                Err(e) => {
                    eprintln!("EventSub WebSocket error: {:?}", e);
                    break;
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn create_eventsub_subscription(&self, session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let token = self.api_client.get_token().await?;
        let channel_id = self.get_channel_id().await?;
        let client_id = self.config.read().await.twitch_client_id.clone().ok_or("Twitch client ID not set")?;

        let subscription = json!({
            "type": "channel.update",
            "version": "1",
            "condition": {
                "broadcaster_user_id": channel_id
            },
            "transport": {
                "method": "websocket",
                "session_id": session_id
            }
        });

        let response = self.http_client
            .post("https://api.twitch.tv/helix/eventsub/subscriptions")
            .header("Client-Id", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .json(&subscription)
            .send()
            .await?;

        if response.status().is_success() {
            println!("EventSub subscription created successfully");
        } else {
            let error_body = response.text().await?;
            return Err(format!("Failed to create EventSub subscription: {}", error_body).into());
        }

        Ok(())
    }

    async fn get_channel_id(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.read().await;
        let channel_name = config.twitch_channel_to_join.clone().ok_or("Channel name not set")?;
        drop(config);

        let user_info = self.api_client.get_user_info(&channel_name).await?;
        let channel_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get channel ID")?.to_string();

        Ok(channel_id)
    }
}