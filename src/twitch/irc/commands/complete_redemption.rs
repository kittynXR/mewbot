// src/twitch/irc/commands/complete_redemption.rs

use crate::twitch::eventsub::events::redemptions::RedemptionManager;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use std::future::Future;
use std::pin::Pin;

pub fn handle_complete_redemption<'a>(
    msg: &'a PrivmsgMessage,
    client: &'a Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &'a str,
    redemption_manager: &'a Arc<RedemptionManager>,
    params: &'a [&'a str],
) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
    Box::pin(async move {
        if params.is_empty() {
            client.say(channel.to_string(), "Usage: !complete <redemption_id>".to_string()).await?;
            return Ok(());
        }

        let redemption_id = params[0];

        match redemption_manager.complete_redemption(redemption_id, &msg.sender.id).await {
            Ok(_) => {
                client.say(channel.to_string(), format!("Redemption {} marked as complete", redemption_id)).await?;
            },
            Err(e) => {
                client.say(channel.to_string(), format!("Error completing redemption: {}", e)).await?;
            }
        }

        Ok(())
    })
}