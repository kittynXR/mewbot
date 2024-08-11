use crate::twitch::utils::get_stream_uptime;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::irc::TwitchBotClient;
use std::sync::Arc;
use tokio::sync::RwLock;
use twitch_irc::message::PrivmsgMessage;
use crate::discord::UserLinks;
use crate::storage::StorageClient;

pub async fn handle_uptime(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    api_client: &Arc<TwitchAPIClient>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
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
            client.send_message(channel, &response).await?;
        },
        Err(e) => {
            eprintln!("Error getting stream uptime: {:?}", e);
            client.send_message(channel, "Sorry, I couldn't retrieve the stream uptime. Please try again later.").await?;
        }
    }
    Ok(())
}