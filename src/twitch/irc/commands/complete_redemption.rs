// src/twitch/irc/commands/complete_redemption.rs

use crate::twitch::redeems::RedeemManager;
use twitch_irc::message::PrivmsgMessage;
use crate::twitch::irc::TwitchBotClient;
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use log::{debug, error, warn};
use tokio::sync::RwLock;
use crate::storage::StorageClient;
use crate::discord::UserLinks;


pub fn handle_complete_redemption<'a>(
    msg: &'a PrivmsgMessage,
    client: &'a Arc<TwitchBotClient>,
    channel: &'a str,
    redeem_manager: &'a Arc<RwLock<RedeemManager>>,
    _storage: &'a Arc<RwLock<StorageClient>>,
    _user_links: &'a Arc<UserLinks>,
    params: &'a [&'a str],
) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {

    Box::pin(async move {
        debug!("Handling complete redemption command. Sender: {}, Params: {:?}", msg.sender.name, params);

        if params.is_empty() {
            warn!("No redemption ID provided");
            client.send_message(channel, "Usage: !complete <redemption_id>").await?;
            return Ok(());
        }

        let redemption_id = params[0];
        debug!("Attempting to complete redemption: {}", redemption_id);

        // Acquire a write lock on the RedeemManager
        let mut redeem_manager = redeem_manager.write().await;

        match redeem_manager.complete_redemption(redemption_id).await {
            Ok(_) => {
                debug!("Redemption {} completed successfully", redemption_id);
                client.send_message(channel, &format!("Redemption {} marked as complete", redemption_id)).await?;
            },
            Err(e) => {
                error!("Error completing redemption {}: {}", redemption_id, e);
                client.send_message(channel, &format!("Error completing redemption: {}", e)).await?;
            }
        }

        Ok(())
    })
}