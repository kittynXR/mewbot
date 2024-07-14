use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::TwitchIRCClient;
use twitch_irc::ClientConfig;
use twitch_irc::SecureTCPTransport;
use twitch_irc::message::ServerMessage;
use std::sync::Arc;
use std::io::{self, Write};
use crate::Config;

pub struct TwitchClient {
    pub client: Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    pub incoming_messages: tokio::sync::mpsc::UnboundedReceiver<ServerMessage>,
}

impl TwitchClient {
    pub fn new(config: &mut Config) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let (username, token, channel) = match (&config.twitch_username, &config.twitch_token, &config.twitch_channel) {
            (Some(u), Some(t), Some(c)) => (u.clone(), t.clone(), c.clone()),
            _ => {
                let (u, t, c) = Self::prompt_for_credentials()?;
                config.set_twitch_credentials(u.clone(), t.clone(), c.clone())?;
                (u, t, c)
            }
        };

        let irc_config = ClientConfig::new_simple(
            StaticLoginCredentials::new(username, Some(token))
        );
        let (incoming_messages, client) =
            TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(irc_config);

        let client = Arc::new(client);

        // Join the channel
        client.join(channel)?;

        Ok(TwitchClient { client, incoming_messages })
    }

    fn prompt_for_credentials() -> Result<(String, String, String), Box<dyn std::error::Error + Send + Sync>> {
        print!("Enter your Twitch username: ");
        io::stdout().flush()?;
        let mut username = String::new();
        io::stdin().read_line(&mut username)?;
        let username = username.trim().to_string();

        print!("Enter your Twitch token: ");
        io::stdout().flush()?;
        let mut token = String::new();
        io::stdin().read_line(&mut token)?;
        let token = token.trim().to_string();

        print!("Enter the Twitch channel to join: ");
        io::stdout().flush()?;
        let mut channel = String::new();
        io::stdin().read_line(&mut channel)?;
        let channel = channel.trim().to_string();

        Ok((username, token, channel))
    }
}