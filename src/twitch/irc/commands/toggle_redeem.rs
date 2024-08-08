use std::sync::Arc;
use tokio::sync::RwLock;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::{SecureTCPTransport, TwitchIRCClient};
use crate::twitch::irc::client::TwitchIRCClientType;
use crate::twitch::redeems::RedeemManager;
use crate::storage::StorageClient;
use crate::discord::UserLinks;



pub async fn handle_toggle_redeem(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {    if params.is_empty() {
        client.say(channel.to_string(), "Usage: !toggleredeem <redeem_name>".parse().unwrap()).await?;
        return Ok(());
    }

    let redeem_name = params.join(" ");
    let mut manager = redeem_manager.write().await;
    let mut updated = false;
    let mut new_status = false;

    {
        let mut handlers = manager.handlers_by_id.write().await;
        if let Some(settings) = handlers.values_mut().find(|s| s.title == redeem_name) {
            settings.active = !settings.active;
            new_status = settings.active;
            updated = true;
        }
    }

    if updated {
        manager.update_twitch_redeems().await?;
        let status = if new_status { "enabled" } else { "disabled" };
        client.say(channel.to_string(), format!("Redeem '{}' has been {}", redeem_name, status)).await?;
    } else {
        client.say(channel.to_string(), format!("Redeem '{}' not found", redeem_name)).await?;
    }

    Ok(())
}