use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;

pub async fn handle_hello(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let response = format!("Hello, {}!", msg.sender.name);
    client.say(channel.to_string(), response).await?;
    Ok(())
}