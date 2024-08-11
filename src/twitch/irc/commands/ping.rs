use crate::twitch::irc::TwitchBotClient;
use std::sync::Arc;
use twitch_irc::message::PrivmsgMessage;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use tokio::sync::RwLock;

pub async fn handle_ping(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client.send_message(channel, "Pong!").await?;
    Ok(())
}