use std::sync::Arc;
use tokio::sync::RwLock;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::{SecureTCPTransport, TwitchIRCClient};
use twitch_irc::login::StaticLoginCredentials;
use crate::twitch::irc::client::TwitchIRCClientType;
use crate::twitch::redeems::RedeemManager;
use crate::storage::StorageClient;
use crate::discord::UserLinks;

pub async fn handle_set_offline_redeem(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {    if params.len() != 2 {
        client.say(channel.to_string(), "Usage: !setofflineredeem <redeem_name> <true/false>".parse().unwrap()).await?;
        return Ok(());
    }

    let redeem_name = params[0];
    let offline_status = params[1].parse::<bool>().map_err(|_| "Invalid boolean value")?;

    let mut manager = redeem_manager.write().await;
    let mut updated = false;

    {
        let mut handlers = manager.handlers_by_id.write().await;
        if let Some(settings) = handlers.values_mut().find(|s| s.title == redeem_name) {
            settings.offline_chat_redeem = offline_status;
            updated = true;
        }
    }

    if updated {
        manager.save_settings().await?;
        manager.update_twitch_redeems().await?;
        client.say(channel.to_string(), format!("Offline chat status for '{}' set to: {}", redeem_name, offline_status)).await?;
    } else {
        client.say(channel.to_string(), format!("Redeem '{}' not found", redeem_name)).await?;
    }

    Ok(())
}