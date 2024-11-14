use std::error::Error;
use crate::config::Config;
use crate::twitch::{TwitchManager};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::Message, WebSocketStream};
use reqwest::Client;
use super::handlers;
use std::time::Duration;
use tokio::time::{interval, timeout, Instant};
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::SinkExt;
use serde::ser::StdError;
use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};
use log::{debug, error, info, trace, warn};
use tungstenite::protocol::CloseFrame;
use crate::osc::models::OSCConfig;
use crate::osc::osc_config::OSCConfigurations;

type BoxedError = Box<dyn StdError + Send + Sync>;
type WebSocketTx = SplitSink<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, Message>;
type WebSocketRx = SplitStream<WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>>;

pub struct TwitchEventSubClient {
    twitch_manager: Arc<TwitchManager>,
    http_client: Client,
    ws_tx: Mutex<Option<WebSocketTx>>,
    ws_rx: Mutex<Option<WebSocketRx>>,
    osc_configs: Arc<RwLock<OSCConfigurations>>,
    config: Arc<Config>,
    reconnect_attempts: Arc<AtomicUsize>,
    max_reconnect_attempts: usize,
    base_delay: Duration,
    max_delay: Duration,
    last_keepalive: Arc<Mutex<Instant>>,
    keepalive_timeout: Arc<Mutex<Duration>>,
    consecutive_keepalive_failures: Arc<AtomicUsize>,
    max_consecutive_keepalive_failures: usize,
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
        osc_configs: Arc<RwLock<OSCConfigurations>>,
    ) -> Self {
        let client = Self {
            twitch_manager: twitch_manager.clone(),
            http_client: Client::new(),
            ws_tx: Mutex::new(None),
            ws_rx: Mutex::new(None),
            osc_configs,
            config: twitch_manager.config.clone(),
            reconnect_attempts: Arc::new(AtomicUsize::new(0)),
            max_reconnect_attempts: 10,
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            last_keepalive: Arc::new(Mutex::new(Instant::now())),
            keepalive_timeout: Arc::new(Mutex::new(Duration::from_secs(60))),
            consecutive_keepalive_failures: Arc::new(AtomicUsize::new(0)),
            max_consecutive_keepalive_failures: 2,
        };

        client
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn StdError + Send + Sync>> {
        info!("Shutting down TwitchEventSubClient...");

        let shutdown_timeout = Duration::from_secs(10);

        let close_websocket = async {
            if let Some(mut ws_tx) = self.ws_tx.lock().await.take() {
                // Send close frame to the server
                if let Err(e) = ws_tx.close().await {
                    warn!("Error sending close frame: {:?}", e);
                }
            }
            // Remove the WebSocket receiver to stop any ongoing message processing
            self.ws_rx.lock().await.take();

            Ok::<(), Box<dyn StdError + Send + Sync>>(())
        };
        match timeout(shutdown_timeout, close_websocket).await {
            Ok(Ok(_)) => info!("WebSocket connection closed successfully"),
            Ok(Err(e)) => warn!("Error closing WebSocket connection: {:?}", e),
            Err(_) => warn!("Timeout while closing WebSocket connection"),
        }
        info!("TwitchEventSubClient shutdown complete.");
        Ok(())
    }

    pub async fn connect_and_listen(&self) -> Result<(), Box<dyn StdError + Send + Sync>> {
        loop {
            match self.connect_websocket("wss://eventsub.wss.twitch.tv/ws").await {
                Ok(()) => {
                    self.reconnect_attempts.store(0, Ordering::SeqCst);
                    self.consecutive_keepalive_failures.store(0, Ordering::SeqCst);
                    if let Err(e) = self.listen_for_messages().await {
                        error!("Error in message handling: {:?}", e);
                    }
                    warn!("Connection lost. Attempting to reconnect...");
                }
                Err(e) => {
                    error!("WebSocket connection error: {:?}", e);
                }
            }

            let attempts = self.reconnect_attempts.fetch_add(1, Ordering::SeqCst);
            if attempts >= self.max_reconnect_attempts {
                error!("Max reconnection attempts reached. Exiting.");
                return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Max reconnection attempts reached")));
            }

            let backoff_duration = self.calculate_backoff(attempts);
            warn!("Attempting to reconnect (attempt {}/{}) after {:?}", attempts + 1, self.max_reconnect_attempts, backoff_duration);
            tokio::time::sleep(backoff_duration).await;
        }
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
        let mut keepalive_check_interval = interval(Duration::from_secs(15));

        loop {
            tokio::select! {
                message = ws_rx.next() => {
                    match message {
                        Some(Ok(msg)) => self.handle_message(msg).await?,
                        Some(Err(e)) => {
                            error!("EventSub WebSocket error: {:?}", e);
                            return Err(Box::new(e));
                        }
                        None => {
                            warn!("EventSub WebSocket stream ended");
                            return Ok(());
                        }
                    }
                }
                _ = keepalive_check_interval.tick() => {
                    if let Err(e) = self.check_keepalive().await {
                        warn!("Keepalive check failed: {:?}", e);
                        if self.consecutive_keepalive_failures.fetch_add(1, Ordering::SeqCst) + 1 >= self.max_consecutive_keepalive_failures {
                            error!("Max consecutive keepalive failures reached. Forcing reconnection.");
                            return Err("Max keepalive failures".into());
                        }
                    } else {
                        self.consecutive_keepalive_failures.store(0, Ordering::SeqCst);
                    }
                }
            }
        }
    }

    async fn handle_message(&self, message: Message) -> Result<(), Box<dyn StdError + Send + Sync>> {
        match message {
            Message::Text(text) => {
                let response: Value = serde_json::from_str(&text)?;
                match response["metadata"]["message_type"].as_str() {
                    Some("session_welcome") => self.handle_welcome_message(&response).await?,
                    Some("session_keepalive") => self.handle_keepalive_message(&response).await?,
                    Some("notification") => self.handle_notification(&response).await?,
                    Some("session_reconnect") => return self.handle_reconnect_message(&response).await,
                    Some("revocation") => self.handle_revocation(&response).await?,
                    _ => warn!("Received unhandled message type: {}", response["metadata"]["message_type"]),
                }
            }
            Message::Close(frame) => return self.handle_close_message(frame).await,
            _ => debug!("Received non-text message: {:?}", message),
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
            if let Some(keepalive_timeout) = session["keepalive_timeout_seconds"].as_u64() {
                let mut timeout = self.keepalive_timeout.lock().await;
                *timeout = Duration::from_secs(keepalive_timeout);
            }
        }
        *self.last_keepalive.lock().await = Instant::now();
        Ok(())
    }

    async fn handle_keepalive_message(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        trace!("Received EventSub keepalive: {:?}", response);
        let mut last_keepalive = self.last_keepalive.lock().await;
        *last_keepalive = Instant::now();
        self.consecutive_keepalive_failures.store(0, Ordering::SeqCst);
        Ok(())
    }

    async fn check_keepalive(&self) -> Result<(), Box<dyn StdError + Send + Sync>> {
        let last_keepalive = *self.last_keepalive.lock().await;
        let timeout = *self.keepalive_timeout.lock().await;
        let elapsed = last_keepalive.elapsed();

        if elapsed > timeout * 3 {  // Allow for triple the timeout before considering it an error
            error!("Keepalive timeout exceeded. Last keepalive: {:?} ago, Timeout: {:?}", elapsed, timeout);
            Err("Keepalive timeout".into())
        } else {
            debug!("Keepalive check passed. Last keepalive: {:?} ago, Timeout: {:?}", elapsed, timeout);
            Ok(())
        }
    }

    #[allow(dead_code)]
    async fn handle_ping_message(&self, data: Vec<u8>) -> Result<(), Box<dyn StdError + Send + Sync>> {
        if let Some(ws_tx) = &mut *self.ws_tx.lock().await {
            ws_tx.send(Message::Pong(data)).await?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    async fn handle_reconnect_message(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        if let Some(new_url) = response["payload"]["session"]["reconnect_url"].as_str() {
            warn!("Received reconnect message. New URL: {}", new_url);
            self.handle_reconnect(new_url.to_string()).await
        } else {
            Err("Invalid reconnect message".into())
        }
    }

    async fn handle_close_message(&self, frame: Option<CloseFrame<'_>>) -> Result<(), Box<dyn StdError + Send + Sync>> {
        match frame {
            Some(frame) => {
                warn!("EventSub WebSocket closed with code {}: {}", frame.code, frame.reason);
                match frame.code.into() {
                    4000 => error!("Internal server error"),
                    4001 => error!("Client sent inbound traffic"),
                    4002 => error!("Client failed ping-pong"),
                    4003 => error!("Connection unused"),
                    4004 => error!("Reconnect grace time expired"),
                    4005 => warn!("Network timeout"),
                    4006 => warn!("Network error"),
                    4007 => error!("Invalid reconnect"),
                    _ => warn!("Unknown close code"),
                }
            }
            None => warn!("EventSub WebSocket closed without a frame"),
        }
        Ok(())
    }

    async fn handle_notification(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        info!("Received notification: {:?}", response);

        let message = serde_json::to_string(response)?;
        if let Err(e) = handlers::handle_message(&message, &self.twitch_manager).await {
            error!("Error handling EventSub message: {:?}", e);
        }

        Ok(())
    }

    async fn handle_revocation(&self, response: &Value) -> Result<(), Box<dyn StdError + Send + Sync>> {
        warn!("Received revocation: {:?}", response);
        // Implement revocation handling logic here
        Ok(())
    }

    #[allow(dead_code)]
    async fn send_ping(&self) -> Result<(), Box<dyn StdError + Send + Sync>> {
        if let Some(ws_tx) = &mut *self.ws_tx.lock().await {
            ws_tx.send(Message::Ping(vec![])).await?;
        }
        Ok(())
    }

    pub async fn refresh_token_periodically(&self) -> Result<(), BoxedError> {
        if let Err(e) = self.twitch_manager.api_client.refresh_token().await {
            error!("Failed to refresh token: {:?}", e);
            return Err(e.into());
        }
        Ok(())
    }

    fn calculate_backoff(&self, attempt: usize) -> Duration {
        let delay = self.base_delay * 2u32.pow(attempt as u32);
        std::cmp::min(delay, self.max_delay)
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
            ("channel.cheer", "1", json!({
                "broadcaster_user_id": channel_id
        })),
            ("channel.channel_points_custom_reward_redemption.add", "1", json!({
            "broadcaster_user_id": channel_id
        })),
            ("channel.channel_points_custom_reward_redemption.update", "1", json!({
            "broadcaster_user_id": channel_id
        })),
            ("channel.ad_break.begin", "1", json!({ // New subscription
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

    pub async fn add_osc_config(&self, event_type: String, config: OSCConfig) {
        let mut configs = self.osc_configs.write().await;
        configs.add_config(&event_type, config);
        configs.save("osc_config.json").unwrap_or_else(|e| eprintln!("Failed to save OSC configs: {}", e));
    }
}