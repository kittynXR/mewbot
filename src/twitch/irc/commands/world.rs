use crate::vrchat::models::World;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use twitch_irc::message::PrivmsgMessage;
use crate::twitch::irc::TwitchBotClient;
use crate::discord::UserLinks;
use crate::storage::StorageClient;
use crate::vrchat::{VRChatClient, VRChatManager};

pub async fn handle_world(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    world_info: &Arc<Mutex<Option<World>>>,
    storage: &Arc<RwLock<StorageClient>>,
    user_links: &Arc<UserLinks>,
    vrchat_manager: &Arc<VRChatManager>,
    is_stream_online: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !is_stream_online {
        client.send_message(channel, "The world status is not available while the stream is offline.").await?;
        return Ok(());
    }

    if !vrchat_manager.is_online().await {
        client.send_message(channel, "The VRChat status is currently offline.").await?;
        return Ok(());
    }

    let world = vrchat_manager.get_current_world().await?;

    // First message with original information
    let first_message = format!(
        "Current World: {} | Author: {} | Capacity: {} | Description: {} | Status: {}",
        world.name, world.author_name, world.capacity, world.description, world.release_status
    );
    client.send_message(channel, &first_message).await?;

    // Second message with dates and world link
    let world_link = format!("https://vrchat.com/home/world/{}", world.id);
    let second_message = format!(
        "Published: {} | Last Updated: {} | World Link: {}",
        world.created_at.format("%Y-%m-%d"),
        world.updated_at.format("%Y-%m-%d"),
        world_link
    );
    client.send_message(channel, &second_message).await?;

    Ok(())
}