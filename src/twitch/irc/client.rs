use crate::config::Config;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::TwitchIRCClient as TwitchIRC;
use twitch_irc::ClientConfig;
use twitch_irc::SecureTCPTransport;
use twitch_irc::message::ServerMessage;

pub struct TwitchIRCClient {
    pub client: Arc<TwitchIRC<SecureTCPTransport, StaticLoginCredentials>>,
    pub message_receiver: mpsc::UnboundedReceiver<ServerMessage>,
}

impl TwitchIRCClient {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config = config.read().await;

        let username = config.twitch_bot_username.clone().ok_or("Twitch IRC username not set")?;
        let oauth_token = config.twitch_irc_oauth_token.clone().ok_or("Twitch IRC OAuth token not set")?;
        let channel = config.twitch_channel_to_join.clone().ok_or("Twitch channel to join not set")?;

        println!("Twitch IRC username: {}", username);
        println!("Twitch IRC OAuth token (first 10 chars): {}...", &oauth_token[..10]);
        println!("Twitch channel to join: {}", channel);

        let client_config = ClientConfig::new_simple(
            StaticLoginCredentials::new(username, Some(oauth_token))
        );

        let (incoming_messages, client) =
            TwitchIRC::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);

        let client = Arc::new(client);

        // Join the channel
        client.join(channel)?;

        Ok(TwitchIRCClient {
            client,
            message_receiver: incoming_messages,
        })
    }
}