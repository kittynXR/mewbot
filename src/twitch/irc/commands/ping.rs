use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use twitch_irc::message::PrivmsgMessage;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use tokio::sync::RwLock;

pub async fn handle_ping(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client.say(channel.to_string(), "Pong!".to_string()).await?;
    Ok(())
}