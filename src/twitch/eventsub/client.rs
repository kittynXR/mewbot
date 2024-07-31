use std::error::Error;
use crate::config::Config;
use crate::twitch::TwitchAPIClient;
use crate::ai::AIClient;
use crate::osc::VRChatOSC;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
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
use tokio::time::timeout;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;
use serde::ser::StdError;
use std::fmt;

type BoxedError = Box<dyn StdError + Send + Sync>;
type WebSocketTx = SplitSink<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>;
type WebSocketRx = SplitStream<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>;

pub struct TwitchEventSubClient {
    config: Arc<RwLock<Config>>,
    api_client: Arc<TwitchAPIClient>,
    irc_client: Arc<TwitchIRCClientType>,
    http_client: Client,
    channel: String,
    pub(crate) redeem_manager: Arc<RwLock<RedeemManager>>,
    ws_tx: Mutex<Option<WebSocketTx>>,
    ws_rx: Mutex<Option<WebSocketRx>>,
    ai_client: Option<Arc<AIClient>>,
    osc_client: Option<Arc<VRChatOSC>>,
}

#[derive(Debug)]
pub struct TwitchEventSubError {
    pub message: String,
}

impl fmt::Display for TwitchEventSubError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for TwitchEventSubError {}

impl TwitchEventSubClient {
    pub fn new(
        config: Arc<RwLock<Config>>,
        api_client: Arc<TwitchAPIClient>,
        irc_client: Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
        channel: String,
        redeem_manager: Arc<RwLock<RedeemManager>>,
        ai_client: Option<Arc<AIClient>>,
        osc_client: Option<Arc<VRChatOSC>>,
    ) -> Self {
        println!("Debug: Creating new TwitchEventSubClient");
        Self {
            config,
            api_client,
            irc_client,
            http_client: Client::new(),
            channel,
            redeem_manager,
            ws_tx: Mutex::new(None),
            ws_rx: Mutex::new(None),
            ai_client,
            osc_client,
        }
    }

    pub async fn connect_and_listen(&self) -> Result<(), Box<dyn StdError + Send + Sync>> {
        let url = "wss://eventsub.wss.twitch.tv/ws".to_string();
        let mut reconnect_attempt = 0;
        let max_reconnect_attempts = 5;

        loop {
            match self.connect_websocket(&url).await {
                Ok(()) => {
                    reconnect_attempt = 0;
                    if let Err(e) = self.listen_for_messages().await {
                        eprintln!("Error in message handling: {:?}", e);
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

    async fn connect_websocket(&self, url: &str) -> Result<(), Box<dyn StdError + Send + Sync>> {
        let (ws_stream, _) = connect_async(url).await?;
        println!("EventSub WebSocket connected to {}", url);
        let (ws_tx, ws_rx) = ws_stream.split();
        *self.ws_tx.lock().await = Some(ws_tx);
        *self.ws_rx.lock().await = Some(ws_rx);
        Ok(())
    }

    async fn listen_for_messages(&self) -> Result<(), Box<dyn StdError + Send + Sync>> {
        let mut ws_rx = self.ws_rx.lock().await.take().expect("WebSocket receive stream not initialized");

        while let Some(message) = ws_rx.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    let response: Value = serde_json::from_str(&text)?;
                    match response["metadata"]["message_type"].as_str() {
                        Some("session_welcome") => {
                            self.handle_welcome_message(&response).await?;
                        }
                        Some("session_keepalive") => {
                            self.handle_keepalive_message(&response).await?;
                        }
                        Some("session_reconnect") => {
                            if let Some(new_url) = response["payload"]["session"]["reconnect_url"].as_str() {
                                println!("Received reconnect message. New URL: {}", new_url);
                                self.handle_reconnect(new_url.to_string()).await?;
                                return Ok(());
                            }
                        }
                        Some("notification") => {
                            self.handle_notification(&response).await?;
                        }
                        Some("revocation") => {
                            self.handle_revocation(&response).await?;
                        }
                        _ => {
                            println!("Received unhandled message type: {}", response["metadata"]["message_type"]);
                        }
                    }
                }
                Ok(Message::Close(frame)) => {
                    println!("EventSub WebSocket closed with frame: {:?}", frame);
                    return Ok(());
                }
                Ok(Message::Ping(data)) => {
                    if let Some(ws_tx) = &mut *self.ws_tx.lock().await {
                        ws_tx.send(Message::Pong(data)).await?;
                    }
                }
                Err(e) => {
                    eprintln!("EventSub WebSocket error: {:?}", e);
                    return Err(Box::new(e) as Box<dyn StdError + Send + Sync>);
                }
                _ => {
                    let config = self.config.clone();
                    log_verbose!(config, "Received non-text message: {:?}", message);
                }
            }
        }

        Ok(())
    }

    async fn handle_reconnect(&self, reconnect_url: String) -> Result<(), Box<dyn StdError + Send + Sync>> {
        println!("Handling reconnect to URL: {}", reconnect_url);

        // Start a 30-second timer
        let reconnect_result = timeout(Duration::from_secs(30), async {
            // Close the old connection
            if let Some(ws_tx) = &mut *self.ws_tx.lock().await {
                ws_tx.close().await?;
            }

            // Connect to the new URL
            self.connect_websocket(&reconnect_url).await
        }).await;

        match reconnect_result {
            Ok(Ok(())) => {
                println!("Successfully reconnected to new URL");
                Ok(())
            }
            Ok(Err(e)) => {
                eprintln!("Error during reconnection: {:?}", e);
                Err(e)
            }
            Err(_) => {
                eprintln!("Reconnection timed out");
                Err(Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "Reconnection timed out")))
            }
        }
    }

    async fn handle_welcome_message(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        println!("Received welcome message: {:?}", response);
        if let Some(session) = response["payload"]["session"].as_object() {
            if let Some(session_id) = session["id"].as_str() {
                self.create_eventsub_subscription(session_id).await?;
            }
        }
        Ok(())
    }

    async fn handle_keepalive_message(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        let config = self.config.clone();
        log_verbose!(config, "Received EventSub keepalive: {:?}", response);
        Ok(())
    }

    async fn handle_notification(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        println!("Received notification: {:?}", response);

        let message = serde_json::to_string(response)?;
        handlers::handle_message(
            &message,
            &self.irc_client,
            &self.channel,
            &self.api_client,
            self
        ).await?;

        Ok(())
    }

    async fn handle_revocation(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        println!("Received revocation: {:?}", response);
        // Implement revocation handling logic here
        Ok(())
    }


    pub async fn refresh_token_periodically(&self) -> Result<(), BoxedError> {
        if let Err(e) = self.api_client.refresh_token().await {
            eprintln!("Failed to refresh token: {:?}", e);
            return Err(e.into());
        }
        Ok(())
    }

    fn calculate_backoff(&self, attempt: u32) -> Duration {
        let base_delay = 1;
        let max_delay = 60;
        let delay = base_delay * 2u64.pow(attempt - 1);
        Duration::from_secs(delay.min(max_delay))
    }

    pub async fn check_current_stream_status(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let channel_id = self.get_channel_id().await?;
        let stream_info = self.api_client.get_stream_info(&channel_id).await?;

        let is_live = !stream_info["data"].as_array().unwrap_or(&vec![]).is_empty();
        let game_name = if is_live {
            stream_info["data"][0]["game_name"].as_str().unwrap_or("").to_string()
        } else {
            "".to_string()
        };

        self.redeem_manager.write().await.update_stream_status(game_name).await;

        Ok(())
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
}