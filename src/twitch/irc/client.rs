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
        let client_config = ClientConfig::new_simple(
            StaticLoginCredentials::new(username.clone(), Some(oauth_token))
        );

        let (incoming_messages, client) = TwitchIRC::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);

        let client = Arc::new(client);

        for channel in channels {
            client.join(channel)?;
        }

        let irc_client = IRCClient {
            client: client.clone(),
        };

        self.clients.write().await.insert(username.clone(), irc_client);

        // Spawn a task to handle incoming messages for this client
        let message_sender = self.message_sender.clone();
        tokio::spawn(async move {
            Self::handle_client_messages(username, incoming_messages, message_sender).await;
        });

        Ok(())
    }

    async fn handle_client_messages(
        username: String,
        mut incoming_messages: mpsc::UnboundedReceiver<ServerMessage>,
        message_sender: broadcast::Sender<ServerMessage>,
    ) {
        while let Some(message) = incoming_messages.recv().await {
            if let Err(e) = message_sender.send(message) {
                eprintln!("Failed to broadcast message for {}: {:?}", username, e);
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