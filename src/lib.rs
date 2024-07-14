pub mod config;
pub mod twitch;
pub mod vrchat;

use std::sync::Arc;
use tokio::sync::Mutex;

// Re-export the most important types for convenience
pub use config::Config;
pub use twitch::client::TwitchClient;
pub use vrchat::client::VRChatClient;

pub async fn init(mut config: Config) -> Result<(TwitchClient, VRChatClient), Box<dyn std::error::Error + Send + Sync>> {
    let twitch_client = TwitchClient::new(&mut config)?;
    let vrchat_client = VRChatClient::new(&mut config).await?;

    Ok((twitch_client, vrchat_client))
}

pub async fn run(twitch_client: TwitchClient, mut vrchat_client: VRChatClient) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let world_info = Arc::new(Mutex::new(None));

    let vrchat_handle = tokio::spawn(vrchat::websocket::handler(vrchat_client.get_auth_cookie(), world_info.clone(), vrchat_client.get_current_user_id().await?));

    let twitch_handle = tokio::spawn(twitch::handler::run(
        twitch_client.client,
        twitch_client.incoming_messages,
        world_info.clone()
    ));

    let current_user_id = vrchat_client.get_current_user_id().await
        .map_err(|e| format!("Failed to get current user ID: {}", e))?;

    println!("Current user ID: {}", current_user_id);


    // Handle potential errors from both tasks
    let (twitch_result, vrchat_result) = tokio::try_join!(twitch_handle, vrchat_handle)?;
    twitch_result?;
    vrchat_result?;

    Ok(())
}