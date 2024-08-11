use std::sync::Arc;
use tokio::sync::RwLock;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::message::PrivmsgMessage;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::redeems::RedeemManager;
use crate::storage::StorageClient;
use crate::discord::UserLinks;



pub async fn handle_toggle_redeem(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {    if params.is_empty() {
    client.send_message(channel, "Usage: !toggleredeem <redeem_name>").await?;
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
        client.send_message(channel, &format!("Redeem '{}' has been {}", redeem_name, status)).await?;
    } else {
        client.send_message(channel, &format!("Redeem '{}' not found", redeem_name)).await?;
    }

    Ok(())
}