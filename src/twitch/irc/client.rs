use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use log::{debug, error, info, warn};
use tokio::sync::{RwLock, mpsc, broadcast, Mutex};
use tokio::time::sleep;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::TwitchIRCClient as TwitchIRC;
use twitch_irc::ClientConfig;
use twitch_irc::SecureTCPTransport;
use twitch_irc::message::{ServerMessage, ClearChatAction};
use crate::web_ui::websocket::{DashboardState, WebSocketMessage};
use crate::config::{Config, SocialLinks};
use crate::twitch::connection_monitor::ConnectionMonitor;
pub type TwitchIRCClientType = TwitchIRC<SecureTCPTransport, StaticLoginCredentials>;


#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Reconnecting,
}

#[derive(Clone)]
pub struct IRCClient {
    pub client: Arc<TwitchIRCClientType>,
    pub monitor: Arc<Mutex<ConnectionMonitor>>,
    pub state: Arc<RwLock<ConnectionState>>,
    reconnect_attempts: Arc<AtomicU32>,
    channels: Arc<RwLock<Vec<String>>>,
    circuit_breaker: Arc<AtomicU32>,
}

impl IRCClient {
    pub fn new(client: Arc<TwitchIRCClientType>, channels: Vec<String>) -> Self {
        Self {
            client,
            monitor: Arc::new(Mutex::new(ConnectionMonitor::new())),
            state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
            reconnect_attempts: Arc::new(AtomicU32::new(0)),
            channels: Arc::new(RwLock::new(channels)),
            circuit_breaker: Arc::new(AtomicU32::new(0)),
        }
    }

    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Attempting to connect to Twitch IRC");
        let mut state = self.state.write().await;
        if *state == ConnectionState::Connected {
            debug!("Already connected to Twitch IRC");
            return Ok(());
        }

        *state = ConnectionState::Connected;
        drop(state);

        for channel in self.channels.read().await.iter() {
            match self.client.join(channel.clone()) {
                Ok(_) => info!("Successfully joined channel: {}", channel),
                Err(e) => {
                    error!("Failed to join channel {}: {:?}", channel, e);
                    return Err(Box::new(e));
                }
            }
        }

        self.monitor.lock().await.on_connect();
        self.reconnect_attempts.store(0, Ordering::SeqCst);
        self.circuit_breaker.store(0, Ordering::SeqCst);
        info!("Successfully connected to Twitch IRC");
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Disconnecting from Twitch IRC");
        let mut state = self.state.write().await;
        if *state == ConnectionState::Disconnected {
            debug!("Already disconnected from Twitch IRC");
            return Ok(());
        }

        *state = ConnectionState::Disconnected;
        drop(state);

        // Perform the disconnection
        for channel in self.channels.read().await.iter() {
            self.client.part(channel.clone());
            info!("Left channel: {}", channel);
        }

        self.monitor.lock().await.on_disconnect();
        info!("Successfully disconnected from Twitch IRC");
        Ok(())
    }

    pub async fn reconnect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        const MAX_RECONNECT_ATTEMPTS: u32 = 5;
        const CIRCUIT_BREAKER_THRESHOLD: u32 = 3;
        const CIRCUIT_BREAKER_RESET_TIME: Duration = Duration::from_secs(300); // 5 minutes

        let attempts = self.reconnect_attempts.fetch_add(1, Ordering::SeqCst) + 1;
        let circuit_breaker_count = self.circuit_breaker.load(Ordering::SeqCst);

        if circuit_breaker_count >= CIRCUIT_BREAKER_THRESHOLD {
            error!("Circuit breaker activated. Waiting for {} seconds before attempting to reconnect.", CIRCUIT_BREAKER_RESET_TIME.as_secs());
            sleep(CIRCUIT_BREAKER_RESET_TIME).await;
            self.circuit_breaker.store(0, Ordering::SeqCst);
        }

        if attempts > MAX_RECONNECT_ATTEMPTS {
            error!("Max reconnection attempts reached ({}). Abandoning reconnection.", MAX_RECONNECT_ATTEMPTS);
            *self.state.write().await = ConnectionState::Disconnected;
            return Err("Max reconnection attempts reached".into());
        }

        let backoff_duration = Duration::from_secs(2u64.pow(attempts.min(6)));
        warn!("Attempting to reconnect to Twitch IRC in {} seconds (attempt {}/{})",
          backoff_duration.as_secs(), attempts, MAX_RECONNECT_ATTEMPTS);
        sleep(backoff_duration).await;

        *self.state.write().await = ConnectionState::Reconnecting;

        if let Err(e) = self.disconnect().await {
            error!("Error during disconnect phase of reconnection: {:?}", e);
            // Continue with reconnection attempt even if disconnect fails
        }

        match self.connect().await {
            Ok(_) => {
                info!("Successfully reconnected to Twitch IRC");
                self.reconnect_attempts.store(0, Ordering::SeqCst);
                self.circuit_breaker.store(0, Ordering::SeqCst);
                Ok(())
            }
            Err(e) => {
                error!("Failed to reconnect to Twitch IRC: {:?}", e);
                self.circuit_breaker.fetch_add(1, Ordering::SeqCst);
                Err(e)
            }
        }
    }

    pub async fn add_channel(&self, channel: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Adding channel: {}", channel);
        let mut channels = self.channels.write().await;
        if channels.contains(&channel) {
            debug!("Channel {} is already in the list", channel);
            return Ok(());
        }

        match self.client.join(channel.clone()) {
            Ok(_) => {
                channels.push(channel.clone());
                info!("Successfully added and joined channel: {}", channel);
                Ok(())
            }
            Err(e) => {
                error!("Failed to join channel {}: {:?}", channel, e);
                Err(Box::new(e))
            }
        }
    }

    pub async fn remove_channel(&self, channel: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Removing channel: {}", channel);
        let mut channels = self.channels.write().await;
        if let Some(pos) = channels.iter().position(|x| x == channel) {
            self.client.part(channel.to_string());
            channels.remove(pos);
            info!("Successfully removed and left channel: {}", channel);
            Ok(())
        } else {
            debug!("Channel {} is not in the list", channel);
            Ok(())
        }
    }

    pub async fn send_message(&self, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Sending message to channel {}: {}", channel, message);
        match self.client.say(channel.to_string(), message.to_string()).await {
            Ok(_) => {
                debug!("Successfully sent message to channel {}", channel);
                Ok(())
            }
            Err(e) => {
                error!("Failed to send message to channel {}: {:?}", channel, e);
                Err(Box::new(e))
            }
        }
    }

    pub async fn get_state(&self) -> ConnectionState {
        self.state.read().await.clone()
    }

    pub async fn get_monitor_status(&self) -> (Duration, u32) {
        let monitor = self.monitor.lock().await;
        (monitor.total_uptime, monitor.disconnection_count)
    }
}

pub struct TwitchIRCManager {
    clients: RwLock<HashMap<String, IRCClient>>,
    message_sender: broadcast::Sender<ServerMessage>,
    websocket_sender: mpsc::UnboundedSender<WebSocketMessage>,
    social_links: Arc<RwLock<SocialLinks>>,
    dashboard_state: Arc<RwLock<DashboardState>>,
    config: Arc<Config>,
}

impl Default for TwitchIRCManager {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(1000);
        let (websocket_tx, _) = mpsc::unbounded_channel();
        Self {
            clients: RwLock::new(HashMap::new()),
            message_sender: tx,
            websocket_sender: websocket_tx,
            social_links: Arc::new(RwLock::new(SocialLinks::default())),
            dashboard_state: Arc::new(RwLock::new(DashboardState::default())),
            config: Arc::new(Config::default()),
        }
    }
}

impl TwitchIRCManager {
    pub fn new(
        websocket_sender: mpsc::UnboundedSender<WebSocketMessage>,
        social_links: Arc<RwLock<SocialLinks>>,
        dashboard_state: Arc<RwLock<DashboardState>>,
        config: Arc<Config>,
    ) -> Self {
        let (message_sender, _) = broadcast::channel(1000);
        TwitchIRCManager {
            clients: RwLock::new(HashMap::new()),
            message_sender,
            websocket_sender,
            social_links,
            dashboard_state,
            config,
        }
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Shutting down TwitchIRCManager...");
        let clients = self.clients.read().await;
        for (username, client) in clients.iter() {
            info!("Disconnecting client for user: {}", username);
            match tokio::time::timeout(Duration::from_secs(5), client.disconnect()).await {
                Ok(Ok(_)) => info!("Successfully disconnected client for user: {}", username),
                Ok(Err(e)) => warn!("Error disconnecting client for user {}: {:?}", username, e),
                Err(_) => warn!("Timed out while disconnecting client for user: {}", username),
            }
        }
        info!("TwitchIRCManager shutdown complete.");
        Ok(())
    }

    pub async fn add_client(&self, username: String, oauth_token: String, channels: Vec<String>, handle_messages: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Adding Twitch IRC client for user: {}", username);

        let cleaned_oauth_token = oauth_token.trim_start_matches("oauth:").to_string();

        let mut client_config = ClientConfig::new_simple(
            StaticLoginCredentials::new(username.clone(), Some(cleaned_oauth_token))
        );

        client_config.connect_timeout = std::time::Duration::from_secs(30);
        client_config.max_channels_per_connection = 10;
        client_config.max_waiting_messages_per_connection = 100;

        let (incoming_messages, client) = TwitchIRC::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);
        info!("Twitch IRC client created successfully for user: {}", username);

        let client = Arc::new(client);
        let irc_client = IRCClient::new(client.clone(), channels.clone());

        self.clients.write().await.insert(username.clone(), irc_client.clone());
        if handle_messages && username == *self.config.twitch_bot_username.as_ref().unwrap() {
            let message_sender = self.message_sender.clone();
            let websocket_sender = self.websocket_sender.clone();
            let username_clone = username.clone();
            let irc_client_clone = irc_client.clone();
            let dashboard_state = self.dashboard_state.clone();
            let bot_username = self.config.twitch_bot_username.as_ref().unwrap().clone();
            tokio::spawn(async move {
                Self::handle_client_messages(
                    username_clone,
                    incoming_messages,
                    message_sender,
                    websocket_sender,
                    irc_client_clone,
                    dashboard_state,
                    bot_username,
                ).await;
            });
        }

        irc_client.connect().await?;
        self.dashboard_state.write().await.update_twitch_status(true).await;
        info!("Successfully added Twitch IRC client for user: {}", username);
        Ok(())
    }

    pub async fn handle_client_messages(
        username: String,
        mut incoming_messages: mpsc::UnboundedReceiver<ServerMessage>,
        message_sender: broadcast::Sender<ServerMessage>,
        websocket_sender: mpsc::UnboundedSender<WebSocketMessage>,
        irc_client: IRCClient,
        dashboard_state: Arc<RwLock<DashboardState>>,
        bot_username: String,
    ) {
        while let Some(message) = incoming_messages.recv().await {
            debug!("Received message in handle_client_messages for user {}: {:?}", username, message);
            Self::log_message(&username, &message);

            match &message {
                ServerMessage::Privmsg(msg) => {
                    if username == bot_username {
                        let websocket_message = WebSocketMessage {
                            module: "twitch".to_string(),
                            action: "new_message".to_string(),
                            data: serde_json::json!({
                            "message": msg.message_text,
                            "user_id": msg.sender.id,
                            "user_name": msg.sender.name,
                            "channel": msg.channel_login,
                        }),
                        };
                        if let Err(e) = websocket_sender.send(websocket_message) {
                            debug!("Failed to send message to WebSocket: {:?}", e);
                        }

                        // Broadcast the message only for the bot client
                        if let Err(e) = message_sender.send(message.clone()) {
                            debug!("Failed to broadcast message: {:?}", e);
                        }
                    }
                },
                ServerMessage::Whisper(msg) => {
                    if username == bot_username {
                        let websocket_message = WebSocketMessage {
                            module: "twitch".to_string(),
                            action: "new_whisper".to_string(),
                            data: serde_json::json!({
                            "message": msg.message_text,
                            "user_id": msg.sender.id,
                            "user_name": msg.sender.name,
                        }),
                        };
                        if let Err(e) = websocket_sender.send(websocket_message) {
                            error!("Failed to send whisper to WebSocket: {:?}", e);
                        }

                        // Broadcast the whisper only for the bot client
                        if let Err(e) = message_sender.send(message.clone()) {
                            error!("Failed to broadcast whisper: {:?}", e);
                        }
                    }
                },
                ServerMessage::Reconnect(_) => {
                    warn!("Received reconnect message for user: {}", username);
                    irc_client.monitor.lock().await.on_disconnect();
                    dashboard_state.write().await.update_twitch_status(false).await;
                    if let Err(e) = irc_client.reconnect().await {
                        error!("Failed to reconnect for user {}: {:?}", username, e);
                    } else {
                        info!("Successfully reconnected for user: {}", username);
                        dashboard_state.write().await.update_twitch_status(true).await;
                    }
                },
                ServerMessage::Join(msg) => {
                    if username == bot_username {
                        let websocket_message = WebSocketMessage {
                            module: "twitch".to_string(),
                            action: "user_joined".to_string(),
                            data: serde_json::json!({
                            "user_name": msg.user_login,
                            "channel": msg.channel_login,
                        }),
                        };
                        if let Err(e) = websocket_sender.send(websocket_message) {
                            error!("Failed to send join message to WebSocket: {:?}", e);
                        }
                    }
                },
                ServerMessage::Part(msg) => {
                    if username == bot_username {
                        let websocket_message = WebSocketMessage {
                            module: "twitch".to_string(),
                            action: "user_left".to_string(),
                            data: serde_json::json!({
                            "user_name": msg.user_login,
                            "channel": msg.channel_login,
                        }),
                        };
                        if let Err(e) = websocket_sender.send(websocket_message) {
                            error!("Failed to send part message to WebSocket: {:?}", e);
                        }
                    }
                },
                _ => {
                    // Handle other message types if needed
                }
            }

            // Update the connection monitor for specific events
            match &message {
                ServerMessage::Reconnect(_) | ServerMessage::Join(_) | ServerMessage::Part(_) => {
                    let state = irc_client.get_state().await;
                    match state {
                        ConnectionState::Connected => irc_client.monitor.lock().await.on_connect(),
                        ConnectionState::Disconnected => irc_client.monitor.lock().await.on_disconnect(),
                        ConnectionState::Reconnecting => {} // Do nothing while reconnecting
                    }
                }
                _ => {}
            }
        }

        warn!("Exiting handle_client_messages for {}", username);
        irc_client.monitor.lock().await.on_disconnect();
        dashboard_state.write().await.update_twitch_status(false).await;
    }

    pub async fn handle_message(&self, message: WebSocketMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match message.action.as_str() {
            "send_message" => {
                let channel = message.data["channel"].as_str().ok_or("Missing channel")?;
                let content = message.data["content"].as_str().ok_or("Missing content")?;
                let username = message.data["username"].as_str().ok_or("Missing username")?;

                self.send_message(username, channel, content).await?;
                Ok(())
            },
            "join_channel" => {
                let channel = message.data["channel"].as_str().ok_or("Missing channel")?;
                let username = message.data["username"].as_str().ok_or("Missing username")?;

                if let Some(client) = self.get_client(username).await {
                    match client.join(channel.to_string()) {
                        Ok(_) => {
                            info!("Successfully joined channel {} for user {}", channel, username);
                            Ok(())
                        },
                        Err(e) => {
                            error!("Failed to join channel {} for user {}: {:?}", channel, username, e);
                            Err(format!("Failed to join channel: {:?}", e).into())
                        }
                    }
                } else {
                    Err("Client not found".into())
                }
            },
            "leave_channel" => {
                let channel = message.data["channel"].as_str().ok_or("Missing channel")?;
                let username = message.data["username"].as_str().ok_or("Missing username")?;

                if let Some(client) = self.get_client(username).await {
                    client.part(channel.to_string());
                    Ok(())
                } else {
                    Err("Client not found".into())
                }
            },
            "get_channel_info" => {
                let channel = message.data["channel"].as_str().ok_or("Missing channel")?;
                // Implement logic to get channel info
                // This might involve using the Twitch API, which is not part of the IRC client
                // For now, we'll just return a placeholder
                let response = WebSocketMessage {
                    module: "twitch".to_string(),
                    action: "channel_info".to_string(),
                    data: serde_json::json!({
                        "channel": channel,
                        "viewers": 0,
                        "followers": 0,
                        "subscribers": 0,
                    }),
                };
                self.websocket_sender.send(response)?;
                Ok(())
            },
            "get_bot_status" => {
                let username = message.data["username"].as_str().ok_or("Missing username")?;
                let status = if self.get_client(username).await.is_some() {
                    "connected"
                } else {
                    "disconnected"
                };
                let response = WebSocketMessage {
                    module: "twitch".to_string(),
                    action: "bot_status".to_string(),
                    data: serde_json::json!({
                        "username": username,
                        "status": status,
                    }),
                };
                self.websocket_sender.send(response)?;
                Ok(())
            },
            _ => Err(format!("Unknown Twitch IRC action: {}", message.action).into()),
        }
    }

    fn log_message(username: &str, message: &ServerMessage) {
        match message {
            ServerMessage::Privmsg(msg) => {
                debug!("[{}] {}: {}", msg.channel_login, msg.sender.name, msg.message_text);
            },
            ServerMessage::Notice(msg) => {
                debug!("[NOTICE] {}: {}",
                         msg.channel_login.as_deref().unwrap_or("*"),
                         msg.message_text);
            },
            ServerMessage::Join(msg) => {
                debug!("[JOIN] {} joined {}", msg.user_login, msg.channel_login);
            },
            ServerMessage::Part(msg) => {
                debug!("[PART] {} left {}", msg.user_login, msg.channel_login);
            },
            ServerMessage::UserNotice(msg) => {
                debug!("[USER NOTICE] {}: {}",
                         msg.channel_login,
                         msg.message_text.as_deref().unwrap_or(""));
            },
            ServerMessage::GlobalUserState(msg) => {
                debug!("[GLOBAL USER STATE] User: {}", msg.user_id);
            },
            ServerMessage::UserState(msg) => {
                debug!("[USER STATE] Channel: {}, User: {}",
                         msg.channel_login,
                         msg.user_name);
            },
            ServerMessage::RoomState(msg) => {
                debug!("[ROOM STATE] Channel: {}", msg.channel_login);
            },
            ServerMessage::Whisper(msg) => {
                debug!("[WHISPER] From {}: {}", msg.sender.name, msg.message_text);
            },
            ServerMessage::ClearChat(msg) => {
                match &msg.action {
                    ClearChatAction::UserBanned { user_login, user_id } => {
                        debug!("[CLEAR CHAT] User {} (ID: {}) was banned in channel {}",
                                 user_login, user_id, msg.channel_login);
                    },
                    ClearChatAction::UserTimedOut { user_login, user_id, timeout_length } => {
                        debug!("[CLEAR CHAT] User {} (ID: {}) was timed out for {} seconds in channel {}",
                                 user_login, user_id, timeout_length.as_secs(), msg.channel_login);
                    },
                    ClearChatAction::ChatCleared => {
                        debug!("[CLEAR CHAT] All chat was cleared in channel {}", msg.channel_login);
                    },
                }
            },
            ServerMessage::ClearMsg(msg) => {
                debug!("[CLEAR MSG] Message from {} was deleted in channel {}", msg.sender_login, msg.channel_login);
            },
            _ => {
                debug!("[{}] Received: {:?}", username, message);
            }
        }
    }

    pub async fn get_client(&self, username: &str) -> Option<Arc<TwitchIRCClientType>> {
        self.clients.read().await.get(username).map(|client| client.client.clone())
    }

    pub async fn send_message(&self, username: &str, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(client) = self.get_client(username).await {
            client.say(channel.to_string(), message.to_string()).await?;
            Ok(())
        } else {
            Err("Client not found".into())
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.message_sender.subscribe()
    }

    pub async fn get_discord_link(&self) -> String {
        self.social_links.read().await.discord.clone().unwrap_or_default()
    }

    pub async fn get_xdotcom_link(&self) -> String {
        self.social_links.read().await.xdotcom.clone().unwrap_or_default()
    }

    pub async fn get_vrchat_group_link(&self) -> String {
        self.social_links.read().await.vrchat_group.clone().unwrap_or_default()
    }

    pub async fn get_business_url(&self) -> String {
        self.social_links.read().await.business_url.clone().unwrap_or_default()
    }
}