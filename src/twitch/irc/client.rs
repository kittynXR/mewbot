use crate::config::Config;

use std::sync::Arc;

use tokio::sync::{RwLock, mpsc};

use twitch_irc::login::StaticLoginCredentials;

use twitch_irc::TwitchIRCClient as TwitchIRC;

use twitch_irc::ClientConfig;

use twitch_irc::SecureTCPTransport;

use twitch_irc::message::ServerMessage;

use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;

use crate::CustomTwitchIRCClient;



pub type TwitchIRCClientType = ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>;



pub struct TwitchIRCClient {

    pub client: Arc<TwitchIRCClientType>,

    pub message_receiver: mpsc::UnboundedReceiver<ServerMessage>,

}



impl From<&CustomTwitchIRCClient> for TwitchIRCClient {

    fn from(custom_client: &CustomTwitchIRCClient) -> Self {

        TwitchIRCClient {

            client: custom_client.client.clone(),

            message_receiver: tokio::sync::mpsc::unbounded_channel().1, // Create a new dummy receiver

        }

    }

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



    /// Sends a message to the specified Twitch channel.

    ///

    /// # Arguments

    ///

    /// * `channel` - The name of the channel to send the message to.

    /// * `message` - The content of the message to send.

    ///

    /// # Returns

    ///

    /// Returns a Result indicating success or failure of sending the message.

    pub async fn send_message(&self, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {

        self.client.say(channel.to_string(), message.to_string()).await?;

        Ok(())

    }

}