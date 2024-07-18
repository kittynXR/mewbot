use crate::vrchat::models::World;
use std::sync::Arc;
use tokio::sync::Mutex;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;

pub async fn handle_world(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    world_info: &Arc<Mutex<Option<World>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let response = {
        let guard = world_info.lock().await;
        match &*guard {
            Some(world) => format!(
                "Current World: {} | Author: {} | Capacity: {} | Description: {} | Status: {}",
                world.name, world.author_name, world.capacity, world.description, world.release_status
            ),
            None => "No world information available yet.".to_string(),
        }
    };
    client.say(channel.to_string(), response).await?;
    Ok(())
}