use crate::twitch::utils::get_stream_uptime;
use crate::twitch::TwitchAPIClient;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use twitch_irc::message::PrivmsgMessage;

pub async fn handle_uptime(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    api_client: &Arc<TwitchAPIClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match get_stream_uptime(channel, api_client).await {
        Ok(uptime) => {
            let response = match uptime {
                Some(duration) => format!(
                    "Stream has been live for {} hours, {} minutes, and {} seconds",
                    duration.num_hours(),
                    duration.num_minutes() % 60,
                    duration.num_seconds() % 60
                ),
                None => "Stream is currently offline.".to_string(),
            };
            client.say(channel.to_string(), response).await?;
        },
        Err(e) => {
            eprintln!("Error getting stream uptime: {:?}", e);
            client.say(channel.to_string(), "Sorry, I couldn't retrieve the stream uptime. Please try again later.".to_string()).await?;
        }
    }
    Ok(())
}