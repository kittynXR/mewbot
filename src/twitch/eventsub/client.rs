use std::error::Error;
use crate::config::Config;
use crate::twitch::{TwitchAPIClient, TwitchManager};
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
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use std::time::Duration;
use crate::twitch::redeems::{Redemption, RedeemManager, RedemptionResult, RedemptionStatus};
use crate::twitch::irc::client::TwitchIRCClientType;
use tokio::time::timeout;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;
use serde::ser::StdError;
use std::fmt;
use log::{debug, error, info, trace, warn};
use tokio::io::AsyncReadExt;
use crate::osc::models::OSCConfig;
use crate::osc::osc_config::OSCConfigurations;
use crate::twitch::irc::TwitchBotClient;

type BoxedError = Box<dyn StdError + Send + Sync>;
type WebSocketTx = SplitSink<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>;
type WebSocketRx = SplitStream<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>;

pub struct TwitchEventSubClient {
    twitch_manager: Arc<TwitchManager>,
    http_client: Client,
    channel: String,
    ws_tx: Mutex<Option<WebSocketTx>>,
    ws_rx: Mutex<Option<WebSocketRx>>,
    osc_configs: Arc<RwLock<OSCConfigurations>>,
    config: Arc<Config>,
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
        twitch_manager: Arc<TwitchManager>,
        channel: String,
        osc_configs: Arc<RwLock<OSCConfigurations>>,
    ) -> Self {
        Self {
            twitch_manager: twitch_manager.clone(),
            http_client: Client::new(),
            channel,
            ws_tx: Mutex::new(None),
            ws_rx: Mutex::new(None),
            osc_configs,
            config: twitch_manager.config.clone(),
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
                        error!("Error in message handling: {:?}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("WebSocket error: {:?}", e);
                    reconnect_attempt += 1;
                    if reconnect_attempt > max_reconnect_attempts {
                        error!("Max reconnection attempts reached. Exiting.");
                        return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Max reconnection attempts reached")));
                    }
                    let backoff_duration = self.calculate_backoff(reconnect_attempt);
                    warn!("Attempting to reconnect (attempt {}/{}) after {:?}", reconnect_attempt, max_reconnect_attempts, backoff_duration);
                    tokio::time::sleep(backoff_duration).await;
                }
            }
        }

        Ok(())
    }

    async fn connect_websocket(&self, url: &str) -> Result<(), Box<dyn StdError + Send + Sync>> {
        let (ws_stream, _) = connect_async(url).await?;
        info!("EventSub WebSocket connected to {}", url);
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
                                warn!("Received reconnect message. New URL: {}", new_url);
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
                            warn!("Received unhandled message type: {}", response["metadata"]["message_type"]);
                        }
                    }
                }
                Ok(Message::Close(frame)) => {
                    warn!("EventSub WebSocket closed with frame: {:?}", frame);
                    return Ok(());
                }
                Ok(Message::Ping(data)) => {
                    if let Some(ws_tx) = &mut *self.ws_tx.lock().await {
                        ws_tx.send(Message::Pong(data)).await?;
                    }
                }
                Err(e) => {
                    error!("EventSub WebSocket error: {:?}", e);
                    return Err(Box::new(e) as Box<dyn StdError + Send + Sync>);
                }
                _ => {
                    let config = self.twitch_manager.config.clone();
                    debug!("Received non-text message: {:?}", message);
                }
            }
        }

        Ok(())
    }

    async fn handle_reconnect(&self, reconnect_url: String) -> Result<(), Box<dyn StdError + Send + Sync>> {
        warn!("Handling reconnect to URL: {}", reconnect_url);

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
                info!("Successfully reconnected to new URL");
                Ok(())
            }
            Ok(Err(e)) => {
                error!("Error during reconnection: {:?}", e);
                Err(e)
            }
            Err(_) => {
                error!("Reconnection timed out");
                Err(Box::new(std::io::Error::new(std::io::ErrorKind::TimedOut, "Reconnection timed out")))
            }
        }
    }

    async fn handle_welcome_message(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        info!("Received welcome message: {:?}", response);
        if let Some(session) = response["payload"]["session"].as_object() {
            if let Some(session_id) = session["id"].as_str() {
                self.create_eventsub_subscription(session_id).await?;
            }
        }
        Ok(())
    }

    async fn handle_keepalive_message(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        let config = self.twitch_manager.config.clone();
        debug!("Received EventSub keepalive: {:?}", response);
        Ok(())
    }

    async fn handle_notification(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        info!("Received notification: {:?}", response);

        let message = serde_json::to_string(response)?;
        handlers::handle_message(
            &message,
            &self.twitch_manager,
            self,
        ).await?;

        Ok(())
    }

    async fn handle_revocation(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        warn!("Received revocation: {:?}", response);
        // Implement revocation handling logic here
        Ok(())
    }


    pub async fn refresh_token_periodically(&self) -> Result<(), BoxedError> {
        if let Err(e) = self.twitch_manager.api_client.refresh_token().await {
            error!("Failed to refresh token: {:?}", e);
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
        let stream_info = self.twitch_manager.api_client.get_stream_info(&channel_id).await?;

        let is_live = !stream_info["data"].as_array().unwrap_or(&vec![]).is_empty();
        let game_name = if is_live {
            stream_info["data"][0]["game_name"].as_str().unwrap_or("").to_string()
        } else {
            "".to_string()
        };

        // self.twitch_manager.redeem_manager.write().await.update_stream_status(game_name).await;

        Ok(())
    }

    async fn create_eventsub_subscription(&self, session_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let token = self.twitch_manager.api_client.get_token().await?;
        let channel_id = self.get_channel_id().await?;
        let client_id = self.config.twitch_client_id.as_ref().ok_or("Twitch client ID not set")?;

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
                .header("Client-Id", client_id.as_str())
                .header("Authorization", format!("Bearer {}", token))
                .json(&subscription)
                .send()
                .await?;

            if response.status().is_success() {
                info!("EventSub subscription created successfully for {} (version {})", subscription_type, version);
            } else {
                let error_body = response.text().await?;
                error!("Failed to create EventSub subscription for {} (version {}): {}", subscription_type, version, error_body);
            }
        }

        Ok(())
    }

    async fn get_channel_id(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let channel_name = self.config.twitch_channel_to_join.as_ref().ok_or("Channel name not set")?;
        let user_info = self.twitch_manager.api_client.get_user_info(channel_name).await?;
        let channel_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get channel ID")?.to_string();
        Ok(channel_id)
    }

    pub async fn handle_osc_event(&self, event_type: &str, event_data: &Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let configs = self.osc_configs.read().await;
        if let Some(osc_config) = configs.get_config(event_type) {
            if let Some(vrchat_osc) = &self.twitch_manager.vrchat_osc {
                vrchat_osc.send_osc_message(&osc_config.osc_endpoint, &osc_config.osc_type, &osc_config.osc_value)?;
                debug!("Sent OSC message for event: {}", event_type);
            }
        }
        Ok(())
    }

    pub async fn add_osc_config(&self, event_type: String, config: OSCConfig) {
        let mut configs = self.osc_configs.write().await;
        configs.add_config(&event_type, config);
        configs.save("osc_config.json").unwrap_or_else(|e| eprintln!("Failed to save OSC configs: {}", e));
    }
}