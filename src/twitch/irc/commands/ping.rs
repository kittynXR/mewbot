use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use twitch_irc::message::PrivmsgMessage;

pub async fn handle_ping(
    _msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client.say(channel.to_string(), "Pong!".to_string()).await?;
    Ok(())
}