use std::collections::HashMap;
use std::sync::Arc;
use log::{debug, error, info};
use tokio::sync::{RwLock, mpsc, broadcast};
use tokio::sync::broadcast::error::SendError;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::TwitchIRCClient as TwitchIRC;
use twitch_irc::ClientConfig;
use twitch_irc::SecureTCPTransport;
use twitch_irc::message::{ServerMessage, ClearChatAction};
use crate::web_ui::websocket::WebSocketMessage;

pub type TwitchIRCClientType = TwitchIRC<SecureTCPTransport, StaticLoginCredentials>;

pub struct IRCClient {
    pub client: Arc<TwitchIRCClientType>,
}

pub struct TwitchIRCManager {
    clients: RwLock<HashMap<String, IRCClient>>,
    message_sender: broadcast::Sender<ServerMessage>,
    websocket_sender: mpsc::Sender<WebSocketMessage>,
}

impl TwitchIRCManager {
    pub fn new(websocket_sender: mpsc::Sender<WebSocketMessage>) -> Self {
        let (message_sender, _) = broadcast::channel(1000);
        TwitchIRCManager {
            clients: RwLock::new(HashMap::new()),
            message_sender,
            websocket_sender,
        }
    }

    pub async fn add_client(&self, username: String, oauth_token: String, channels: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        debug!("Adding Twitch IRC client for user: {}", username);

        let cleaned_oauth_token = oauth_token.trim_start_matches("oauth:").to_string();

        let client_config = ClientConfig::new_simple(
            StaticLoginCredentials::new(username.clone(), Some(cleaned_oauth_token))
        );

        let (incoming_messages, client) = TwitchIRC::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);

        let client = Arc::new(client);

        for channel in channels.iter() {
            info!("Joining channel: {}", channel);
            client.join(channel.clone())?;
        }

        let irc_client = IRCClient {
            client: client.clone(),
        };

        self.clients.write().await.insert(username.clone(), irc_client);

        // Spawn a task to handle incoming messages for this client
        let message_sender = self.message_sender.clone();
        let websocket_sender = self.websocket_sender.clone();
        let username_clone = username.clone();  // Clone username for the spawned task
        tokio::spawn(async move {
            Self::handle_client_messages(username_clone, incoming_messages, message_sender, websocket_sender).await;
        });

        info!("Successfully added Twitch IRC client for user: {}", username);
        Ok(())
    }

    async fn handle_client_messages(
        username: String,
        mut incoming_messages: mpsc::UnboundedReceiver<ServerMessage>,
        message_sender: broadcast::Sender<ServerMessage>,
        websocket_sender: mpsc::Sender<WebSocketMessage>,
    ) {
        while let Some(message) = incoming_messages.recv().await {
            debug!("Received message in handle_client_messages for user {}: {:?}", username, message);
            info!("Received message in TwitchIRCManager: {:?}", message);
            Self::log_message(&username, &message);

            if let ServerMessage::Privmsg(msg) = &message {
                let websocket_message = WebSocketMessage {
                    message_type: "twitch_message".to_string(),
                    message: Some(msg.message_text.clone()),
                    user_id: Some(msg.sender.id.clone()),
                    destination: None,
                    world: None,
                    additional_streams: None,
                };
                debug!("Sending message to WebSocket: {:?}", websocket_message);
                if let Err(e) = websocket_sender.send(websocket_message).await {
                    error!("Failed to send message to WebSocket: {:?}", e);
                } else {
                    info!("Successfully sent message to WebSocket");
                }
            }

            if let Err(e) = message_sender.send(message) {
                error!("Failed to broadcast message: {:?}", e);
            } else {
                debug!("Successfully broadcasted message");
            }
        }
        info!("Exiting handle_client_messages for {}", username);
    }

    fn log_message(username: &str, message: &ServerMessage) {
        match message {
            ServerMessage::Privmsg(msg) => {
                println!("[{}] {}: {}", msg.channel_login, msg.sender.name, msg.message_text);
            },
            ServerMessage::Notice(msg) => {
                println!("[NOTICE] {}: {}",
                         msg.channel_login.as_deref().unwrap_or("*"),
                         msg.message_text);
            },
            ServerMessage::Join(msg) => {
                println!("[JOIN] {} joined {}", msg.user_login, msg.channel_login);
            },
            ServerMessage::Part(msg) => {
                println!("[PART] {} left {}", msg.user_login, msg.channel_login);
            },
            ServerMessage::UserNotice(msg) => {
                println!("[USER NOTICE] {}: {}",
                         msg.channel_login,
                         msg.message_text.as_deref().unwrap_or(""));
            },
            ServerMessage::GlobalUserState(msg) => {
                println!("[GLOBAL USER STATE] User: {}", msg.user_id);
            },
            ServerMessage::UserState(msg) => {
                println!("[USER STATE] Channel: {}, User: {}",
                         msg.channel_login,
                         msg.user_name);
            },
            ServerMessage::RoomState(msg) => {
                println!("[ROOM STATE] Channel: {}", msg.channel_login);
            },
            ServerMessage::Whisper(msg) => {
                println!("[WHISPER] From {}: {}", msg.sender.name, msg.message_text);
            },
            ServerMessage::ClearChat(msg) => {
                match &msg.action {
                    ClearChatAction::UserBanned { user_login, user_id } => {
                        println!("[CLEAR CHAT] User {} (ID: {}) was banned in channel {}",
                                 user_login, user_id, msg.channel_login);
                    },
                    ClearChatAction::UserTimedOut { user_login, user_id, timeout_length } => {
                        println!("[CLEAR CHAT] User {} (ID: {}) was timed out for {} seconds in channel {}",
                                 user_login, user_id, timeout_length.as_secs(), msg.channel_login);
                    },
                    ClearChatAction::ChatCleared => {
                        println!("[CLEAR CHAT] All chat was cleared in channel {}", msg.channel_login);
                    },
                }
            },
            ServerMessage::ClearMsg(msg) => {
                println!("[CLEAR MSG] Message from {} was deleted in channel {}", msg.sender_login, msg.channel_login);
            },
            _ => {
                println!("[{}] Received: {:?}", username, message);
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
}