use crate::vrchat::models::World;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use crate::discord::UserLinks;
use crate::storage::StorageClient;

pub async fn handle_world(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    world_info: &Arc<Mutex<Option<World>>>,
    storage: &Arc<RwLock<StorageClient>>,
    user_links: &Arc<UserLinks>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let guard = world_info.lock().await;
    match &*guard {
        Some(world) => {
            // First message with original information
            let first_message = format!(
                "Current World: {} | Author: {} | Capacity: {} | Description: {} | Status: {}",
                world.name, world.author_name, world.capacity, world.description, world.release_status
            );
            client.say(channel.to_string(), first_message).await?;

            // Second message with dates and world link
            let world_link = format!("https://vrchat.com/home/world/{}", world.id);
            let second_message = format!(
                "Published: {} | Last Updated: {} | World Link: {}",
                world.created_at.format("%Y-%m-%d"),
                world.updated_at.format("%Y-%m-%d"),
                world_link
            );
            client.say(channel.to_string(), second_message).await?;
        },
        None => {
            client.say(channel.to_string(), "No world information available yet.".to_string()).await?;
        }
    }
    Ok(())
}