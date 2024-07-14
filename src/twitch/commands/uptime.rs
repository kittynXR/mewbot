use crate::twitch::utils::get_stream_uptime;
use twitch_irc::message::ServerMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;

pub async fn handle_uptime(
    msg: &ServerMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let ServerMessage::Privmsg(_) = msg {
        let uptime = get_stream_uptime(channel).await?;
        let response = match uptime {
            Some(duration) => format!("Stream has been live for {} hours, {} minutes, and {} seconds",
                                      duration.num_hours(),
                                      duration.num_minutes() % 60,
                                      duration.num_seconds() % 60),
            None => "Stream is currently offline.".to_string(),
        };
        client.say(channel.to_string(), response).await?;
    }
    Ok(())
}