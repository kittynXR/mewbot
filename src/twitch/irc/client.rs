use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, broadcast};
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::TwitchIRCClient as TwitchIRC;
use twitch_irc::ClientConfig;
use twitch_irc::SecureTCPTransport;
use twitch_irc::message::ServerMessage;

pub type TwitchIRCClientType = TwitchIRC<SecureTCPTransport, StaticLoginCredentials>;

pub struct IRCClient {
    pub client: Arc<TwitchIRCClientType>,
    pub message_receiver: mpsc::UnboundedReceiver<ServerMessage>,
}

pub struct TwitchIRCManager {
    clients: RwLock<HashMap<String, IRCClient>>,
    message_sender: broadcast::Sender<ServerMessage>,
}

impl TwitchIRCManager {
    pub fn new() -> Self {
        let (message_sender, _) = broadcast::channel(100); // Adjust buffer size as needed
        TwitchIRCManager {
            clients: RwLock::new(HashMap::new()),
            message_sender,
        }
    }

    pub async fn add_client(&self, username: String, oauth_token: String, channels: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Adding client for user: {}", username);
        println!("OAuth token (first 14 chars): {}...", &oauth_token[..14]);
        println!("Twitch channels to join: {:?}", channels);

        let client_config = ClientConfig::new_simple(
            StaticLoginCredentials::new(username.clone(), Some(oauth_token))
        );

        let (incoming_messages, client) = TwitchIRC::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);
        let client = Arc::new(client);

        for channel in channels.iter() {
            client.join(channel.clone())?;
            println!("Joined channel {} for user {}", channel, username);
        }

        let (message_sender, message_receiver) = mpsc::unbounded_channel();
        let irc_client = IRCClient {
            client: client.clone(),
            message_receiver: message_receiver,
        };

        self.clients.write().await.insert(username.clone(), irc_client);

        let broadcast_sender = self.message_sender.clone();
        tokio::spawn(async move {
            Self::handle_client_messages(username, incoming_messages, message_sender, broadcast_sender).await;
        });

        Ok(())
    }

    async fn handle_client_messages(
        username: String,
        mut incoming_messages: mpsc::UnboundedReceiver<ServerMessage>,
        message_sender: mpsc::UnboundedSender<ServerMessage>,
        broadcast_sender: broadcast::Sender<ServerMessage>,
    ) {
        while let Some(message) = incoming_messages.recv().await {
            println!("Received message for {}: {:?}", username, message);
            if let Err(e) = broadcast_sender.send(message.clone()) {
                eprintln!("Failed to broadcast message for {}: {:?}", username, e);
            }
            if let Err(e) = message_sender.send(message) {
                eprintln!("Failed to send message to client channel for {}: {:?}", username, e);
            }
        }
    }

    pub async fn get_client(&self, username: &str) -> Option<Arc<TwitchIRCClientType>> {
        self.clients.read().await.get(username).map(|client| client.client.clone())
    }

    pub async fn send_message(&self, username: &str, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Attempting to send message for user '{}' in channel '{}': {}", username, channel, message);

        match self.get_client(username).await {
            Some(client) => {
                match client.say(channel.to_string(), message.to_string()).await {
                    Ok(_) => {
                        println!("Message sent successfully");
                        Ok(())
                    },
                    Err(e) => {
                        eprintln!("Error sending message: {:?}", e);
                        Err(e.into())
                    }
                }
            },
            None => {
                let error_msg = format!("Client not found for user '{}'", username);
                eprintln!("{}", error_msg);
                Err(error_msg.into())
            }
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMessage> {
        self.message_sender.subscribe()
    }
}