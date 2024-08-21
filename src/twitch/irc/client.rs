use std::collections::HashMap;
use std::sync::Arc;
use log::{debug, error, info, warn};
use tokio::sync::{RwLock, mpsc, broadcast, Mutex};
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::TwitchIRCClient as TwitchIRC;
use twitch_irc::ClientConfig;
use twitch_irc::SecureTCPTransport;
use twitch_irc::message::{ServerMessage, ClearChatAction};
use crate::web_ui::websocket::{DashboardState, WebSocketMessage};
use crate::config::SocialLinks;
use crate::twitch::connection_monitor::ConnectionMonitor;

pub type TwitchIRCClientType = TwitchIRC<SecureTCPTransport, StaticLoginCredentials>;

pub struct IRCClient {
    pub client: Arc<TwitchIRCClientType>,
    pub monitor: Arc<Mutex<ConnectionMonitor>>,
}

pub struct TwitchIRCManager {
    clients: RwLock<HashMap<String, IRCClient>>,
    message_sender: broadcast::Sender<ServerMessage>,
    websocket_sender: mpsc::UnboundedSender<WebSocketMessage>,
    social_links: Arc<RwLock<SocialLinks>>,
    dashboard_state: Arc<RwLock<DashboardState>>,
}

impl TwitchIRCManager {
    pub fn new(
        websocket_sender: mpsc::UnboundedSender<WebSocketMessage>,
        social_links: Arc<RwLock<SocialLinks>>,
        dashboard_state: Arc<RwLock<DashboardState>>,
    ) -> Self {
        let (message_sender, _) = broadcast::channel(1000);
        TwitchIRCManager {
            clients: RwLock::new(HashMap::new()),
            message_sender,
            websocket_sender,
            social_links,
            dashboard_state,
        }
    }

    pub async fn add_client(&self, username: String, oauth_token: String, channels: Vec<String>, handle_messages: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Adding Twitch IRC client for user: {}", username);

        let cleaned_oauth_token = oauth_token.trim_start_matches("oauth:").to_string();

        let client_config = ClientConfig::new_simple(
            StaticLoginCredentials::new(username.clone(), Some(cleaned_oauth_token))
        );

        info!("Attempting to create Twitch IRC client for user: {}", username);
        let (incoming_messages, client) = TwitchIRC::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);
        info!("Twitch IRC client created successfully for user: {}", username);

        let client = Arc::new(client);
        let monitor = Arc::new(Mutex::new(ConnectionMonitor::new()));

        for channel in channels.iter() {
            info!("Joining channel: {} for user: {}", channel, username);
            client.join(channel.clone())?;
            info!("Successfully joined channel: {} for user: {}", channel, username);
        }

        let irc_client = IRCClient {
            client: client.clone(),
            monitor: monitor.clone(),
        };

        self.clients.write().await.insert(username.clone(), irc_client);

        if handle_messages {
            let message_sender = self.message_sender.clone();
            let websocket_sender = self.websocket_sender.clone();
            let username_clone = username.clone();
            let monitor_clone = monitor.clone();
            let dashboard_state = self.dashboard_state.clone();
            tokio::spawn(async move {
                Self::handle_client_messages(
                    username_clone,
                    incoming_messages,
                    message_sender,
                    websocket_sender,
                    monitor_clone,
                    dashboard_state,
                ).await;
            });
        }

        monitor.lock().await.on_connect();
        self.dashboard_state.write().await.update_twitch_status(true);
        info!("Successfully added Twitch IRC client for user: {}", username);
        Ok(())
    }

    pub async fn handle_client_messages(
        username: String,
        mut incoming_messages: mpsc::UnboundedReceiver<ServerMessage>,
        message_sender: broadcast::Sender<ServerMessage>,
        websocket_sender: mpsc::UnboundedSender<WebSocketMessage>,
        monitor: Arc<Mutex<ConnectionMonitor>>,
        dashboard_state: Arc<RwLock<DashboardState>>,
    ) {
        while let Some(message) = incoming_messages.recv().await {
            debug!("Received message in handle_client_messages for user {}: {:?}", username, message);
            Self::log_message(&username, &message);

            match &message {
                ServerMessage::Privmsg(msg) => {
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
                        error!("Failed to send message to WebSocket: {:?}", e);
                    }
                },
                ServerMessage::Reconnect(_) => {
                    warn!("Received reconnect message for user: {}", username);
                    monitor.lock().await.on_disconnect();
                    dashboard_state.write().await.update_twitch_status(false);
                },
                _ => {}
            }

            if let Err(e) = message_sender.send(message) {
                error!("Failed to broadcast message: {:?}", e);
            }
        }
        warn!("Exiting handle_client_messages for {}", username);
        monitor.lock().await.on_disconnect();
        dashboard_state.write().await.update_twitch_status(false);
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
                    client.join(channel.to_string());
                    Ok(())
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