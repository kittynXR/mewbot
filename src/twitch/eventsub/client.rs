use crate::config::Config;
use crate::twitch::TwitchAPIClient;
use crate::ai::AIClient;
use crate::osc::VRChatOSC;
use futures_util::StreamExt;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use super::handlers;
use crate::log_verbose;
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use std::time::Duration;
use crate::twitch::redeems::{Redemption, RedeemManager, RedemptionResult, RedemptionStatus};
use crate::twitch::irc::client::TwitchIRCClientType;

pub struct TwitchEventSubClient {
    config: Arc<RwLock<Config>>,
    api_client: Arc<TwitchAPIClient>,
    irc_client: Arc<TwitchIRCClientType>,
    http_client: Client,
    channel: String,
    redeem_manager: Arc<RwLock<RedeemManager>>,
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
        ai_client: Option<Arc<AIClient>>,
        osc_client: Option<Arc<VRChatOSC>>,
        redeem_manager: Arc<RwLock<RedeemManager>>,
    ) -> Self {
        println!("Debug: Creating new TwitchEventSubClient");
        Self {
            config,
            api_client,
            irc_client,
            http_client: Client::new(),
            channel,
            redeem_manager,
        }
    }

    pub async fn connect_and_listen(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut url = "wss://eventsub.wss.twitch.tv/ws".to_string();
        let mut reconnect_attempt = 0;
        let max_reconnect_attempts = 5;

        loop {
            match self.connect_websocket(&url).await {
                Ok(reconnect_url) => {
                    if let Some(new_url) = reconnect_url {
                        println!("Reconnecting to new URL: {}", new_url);
                        url = new_url;
                        reconnect_attempt = 0; // Reset reconnect attempt count on successful connection
                    } else {
                        println!("WebSocket closed normally. Exiting.");
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("WebSocket error: {:?}", e);
                    reconnect_attempt += 1;
                    if reconnect_attempt > max_reconnect_attempts {
                        eprintln!("Max reconnection attempts reached. Exiting.");
                        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Max reconnection attempts reached")));
                    }
                    let backoff_duration = self.calculate_backoff(reconnect_attempt);
                    println!("Attempting to reconnect (attempt {}/{}) after {:?}", reconnect_attempt, max_reconnect_attempts, backoff_duration);
                    tokio::time::sleep(backoff_duration).await;
                }
            }
        }

        Ok(())
    }

    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base_delay = 1;
        let max_delay = 60;
        let delay = base_delay * 2u64.pow(attempt - 1);
        Duration::from_secs(delay.min(max_delay))
    }

    async fn connect_websocket(&self, url: &str) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let (ws_stream, _) = connect_async(url).await?;
        println!("EventSub WebSocket connected to {}", url);

        let (_, mut read) = ws_stream.split();

        let mut session_id = String::new();
        let mut subscriptions_created = false;

        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    let response: WebSocketResponse = serde_json::from_str(&text)?;
                    match response.metadata.message_type.as_str() {
                        "session_welcome" => {
                            if let Some(session) = response.payload.get("session") {
                                session_id = session["id"].as_str().unwrap_or("").to_string();
                                println!("EventSub session established. Session ID: {}", session_id);
                                if !subscriptions_created {
                                    self.create_eventsub_subscription(&session_id).await?;
                                    subscriptions_created = true;
                                }
                            }
                        }
                        "session_keepalive" => {
                            let config = self.config.clone();
                            log_verbose!(config, "Received EventSub keepalive: {}", text);
                        }
                        "session_reconnect" => {
                            if let Some(new_url) = response.payload["session"]["reconnect_url"].as_str() {
                                println!("Received reconnect message. New URL: {}", new_url);
                                return Ok(Some(new_url.to_string()));
                            }
                        }
                        "notification" => {
                            println!("Received notification: {}", text);
                            handlers::handle_message(&text, &self.irc_client, &self.channel, &self.api_client, self).await?;
                        }
                        "revocation" => {
                            println!("Received revocation: {}", text);
                            // Handle revocation (e.g., remove the subscription from our list)
                        }
                        _ => {
                            println!("Received unhandled message type: {}", response.metadata.message_type);
                        }
                    }
                }
                Ok(Message::Close(frame)) => {
                    println!("EventSub WebSocket closed with frame: {:?}", frame);
                    return Ok(None);
                }
                Ok(Message::Ping(_)) => {
                    // Respond with a Pong message
                    // Note: The WebSocket library might handle this automatically
                }
                Err(e) => {
                    eprintln!("EventSub WebSocket error: {:?}", e);
                    return Err(Box::new(e));
                }
                _ => {
                    let config = self.config.clone();
                    log_verbose!(config, "Received non-text message: {:?}", message);
                }
            }
        }

        Ok(None)
    }

    async fn create_eventsub_subscription(&self, session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let token = self.api_client.get_token().await?;
        let channel_id = self.get_channel_id().await?;
        let client_id = self.config.read().await.twitch_client_id.clone().ok_or("Twitch client ID not set")?;

        let subscriptions = vec![
            ("channel.update", "2", json!({
            "broadcaster_user_id": channel_id
        })),
            ("channel.follow", "2", json!({
            "broadcaster_user_id": channel_id,
            "moderator_user_id": channel_id
        })),
            ("channel.raid", "1", json!({
            "to_broadcaster_user_id": channel_id
        })),
            ("channel.shoutout.create", "1", json!({
            "broadcaster_user_id": channel_id,
            "moderator_user_id": channel_id
        })),
            ("channel.shoutout.receive", "1", json!({
            "broadcaster_user_id": channel_id,
            "moderator_user_id": channel_id
        })),
            ("stream.online", "1", json!({
        "broadcaster_user_id": channel_id
    })),
            ("stream.offline", "1", json!({
        "broadcaster_user_id": channel_id
    })),
            ("channel.subscribe", "1", json!({
        "broadcaster_user_id": channel_id
    })),
            ("channel.subscription.message", "1", json!({
        "broadcaster_user_id": channel_id
    })),
            ("channel.subscription.gift", "1", json!({
        "broadcaster_user_id": channel_id
    })),
            ("channel.subscription.end", "1", json!({
        "broadcaster_user_id": channel_id
    })),
            ("channel.channel_points_custom_reward_redemption.add", "1", json!({
            "broadcaster_user_id": channel_id
        })),
            ("channel.channel_points_custom_reward_redemption.update", "1", json!({
    "broadcaster_user_id": channel_id
})),
        ];

        for (subscription_type, version, condition) in subscriptions {
            let subscription = json!({
            "type": subscription_type,
            "version": version,
            "condition": condition,
            "transport": {
                "method": "websocket",
                "session_id": session_id
            }
        });

            let response = self.http_client
                .post("https://api.twitch.tv/helix/eventsub/subscriptions")
                .header("Client-Id", &client_id)
                .header("Authorization", format!("Bearer {}", token))
                .json(&subscription)
                .send()
                .await?;

            if response.status().is_success() {
                println!("EventSub subscription created successfully for {} (version {})", subscription_type, version);
            } else {
                let error_body = response.text().await?;
                eprintln!("Failed to create EventSub subscription for {} (version {}): {}", subscription_type, version, error_body);
            }
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

    pub async fn handle_channel_point_redemption(&self, event: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let redemption = Redemption {
            id: event["id"].as_str().unwrap_or("").to_string(),
            broadcaster_id: event["broadcaster_user_id"].as_str().unwrap_or("").to_string(),
            user_id: event["user_id"].as_str().unwrap_or("").to_string(),
            user_name: event["user_login"].as_str().unwrap_or("").to_string(),
            reward_id: event["reward"]["id"].as_str().unwrap_or("").to_string(),
            reward_title: event["reward"]["title"].as_str().unwrap_or("").to_string(),
            user_input: event["user_input"].as_str().map(|s| s.to_string()),
            status: event["status"].as_str().unwrap_or("").into(),
            queued: false,
            queue_number: None,
            announce_in_chat: false,
        };

        let status: RedemptionStatus = redemption.status.clone();

        match status {
            RedemptionStatus::Unfulfilled => {
                println!("Processing new redemption: {:?}", redemption);
                let result = self.redeem_manager.read().await.handle_redemption(
                    redemption.clone(),
                    self.irc_client.clone(),
                    self.channel.clone()
                ).await;

                if result.success {
                    println!("Redemption handled successfully: {:?}", result);
                } else {
                    eprintln!("Failed to handle redemption: {:?}", result);
                }

                if redemption.announce_in_chat {
                    self.announce_redemption(&redemption, &result).await;
                }
            },
            RedemptionStatus::Fulfilled => {
                println!("Redemption already fulfilled: {:?}", redemption);
            },
            RedemptionStatus::Canceled => {
                println!("Redemption canceled: {:?}", redemption);
                if let Err(e) = self.redeem_manager.write().await.cancel_redemption(&redemption.id).await {
                    eprintln!("Error canceling redemption: {}", e);
                }
            },
        }

        Ok(())
    }

    pub async fn handle_new_channel_point_redemption(&self, event: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let redemption = Redemption {
            id: event["id"].as_str().unwrap_or("").to_string(),
            broadcaster_id: event["broadcaster_user_id"].as_str().unwrap_or("").to_string(),
            user_id: event["user_id"].as_str().unwrap_or("").to_string(),
            user_name: event["user_login"].as_str().unwrap_or("").to_string(),
            reward_id: event["reward"]["id"].as_str().unwrap_or("").to_string(),
            reward_title: event["reward"]["title"].as_str().unwrap_or("").to_string(),
            user_input: event["user_input"].as_str().map(|s| s.to_string()),
            status: event["status"].as_str().unwrap_or("").into(),
            queued: false,
            queue_number: None,
            announce_in_chat: false,
        };

        println!("Processing new redemption: {:?}", redemption);

        let redeem_manager = self.redeem_manager.read().await;
        let result = if redemption.reward_title == "coin game" {
            redeem_manager.handle_coin_game(&redemption, &self.irc_client, &self.channel).await
        } else {
            redeem_manager.handle_redemption(
                redemption.clone(),
                self.irc_client.clone(),
                self.channel.clone()
            ).await
        };

        if result.success {
            println!("Redemption handled successfully: {:?}", result);
        } else {
            eprintln!("Failed to handle redemption: {:?}", result);
        }

        // Only update the status if it's not a coin game redemption
        if redemption.reward_title != "coin game" {
            let status = if result.success { "FULFILLED" } else { "CANCELED" };
            if let Err(e) = self.api_client.update_redemption_status(&redemption.reward_id, &redemption.id, status).await {
                eprintln!("Failed to update redemption status: {:?}", e);
            }
        }

        Ok(())
    }

    pub async fn handle_channel_point_redemption_update(&self, event: &serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let redemption_id = event["id"].as_str().unwrap_or("");
        let status: RedemptionStatus = event["status"].as_str().unwrap_or("").into();

        match status {
            RedemptionStatus::Canceled => {
                println!("Redemption {} canceled", redemption_id);
                if let Err(e) = self.redeem_manager.write().await.cancel_redemption(redemption_id).await {
                    eprintln!("Error canceling redemption: {}", e);
                }
            },
            _ => {
                println!("Unhandled redemption update status: {:?} for redemption {}", status, redemption_id);
            },
        }

        Ok(())
    }

    async fn announce_redemption(&self, redemption: &Redemption, result: &RedemptionResult) {
        let message = match &result.message {
            Some(msg) => format!("{} redeemed {}! {}", redemption.user_name, redemption.reward_title, msg),
            None => format!("{} redeemed {}!", redemption.user_name, redemption.reward_title),
        };

        if let Err(e) = self.irc_client.say(self.channel.clone(), message).await {
            eprintln!("Failed to announce redemption in chat: {}", e);
        }
    }



    pub(crate) async fn refresh_token_periodically(&self) {
        let refresh_interval = Duration::from_secs(3600); // 1 hour in seconds
        loop {
            tokio::time::sleep(refresh_interval).await;
            if let Err(e) = self.api_client.refresh_token().await {
                eprintln!("Failed to refresh token: {:?}", e);
            }
        }
    }
}