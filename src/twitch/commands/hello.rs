use twitch_irc::message::ServerMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;

pub async fn handle_hello(
    msg: &ServerMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if let ServerMessage::Privmsg(privmsg) = msg {
        let user_name = &privmsg.sender.name;
        let response = format!("Hello, {}! Welcome to the stream!", user_name);
        client.say(channel.to_string(), response).await?;
    }
    Ok(())
}