// src/twitch/irc/commands/complete_redemption.rs

use crate::twitch::redeems::RedeemManager;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::RwLock;

pub fn handle_complete_redemption<'a>(
    msg: &'a PrivmsgMessage,
    client: &'a Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &'a str,
    redeem_manager: &'a Arc<RwLock<RedeemManager>>,
    params: &'a [&'a str],
) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
    Box::pin(async move {
        println!("Handling complete redemption command. Sender: {}, Params: {:?}", msg.sender.name, params);

        if params.is_empty() {
            println!("No redemption ID provided");
            client.say(channel.to_string(), "Usage: !complete <redemption_id>".to_string()).await?;
            return Ok(());
        }

        let redemption_id = params[0];
        println!("Attempting to complete redemption: {}", redemption_id);

        // Acquire a write lock on the RedeemManager
        let mut redeem_manager = redeem_manager.write().await;

        match redeem_manager.complete_redemption(redemption_id).await {
            Ok(_) => {
                println!("Redemption {} completed successfully", redemption_id);
                client.say(channel.to_string(), format!("Redemption {} marked as complete", redemption_id)).await?;
            },
            Err(e) => {
                println!("Error completing redemption {}: {}", redemption_id, e);
                client.say(channel.to_string(), format!("Error completing redemption: {}", e)).await?;
            }
        }

        Ok(())
    })
}