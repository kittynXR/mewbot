use std::sync::Arc;
use tokio::sync::RwLock;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::{SecureTCPTransport, TwitchIRCClient};
use twitch_irc::login::StaticLoginCredentials;
use crate::twitch::irc::client::TwitchIRCClientType;
use crate::twitch::redeems::RedeemManager;
use crate::storage::StorageClient;
use crate::discord::UserLinks;


pub async fn handle_set_active_games(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {    if params.len() < 2 {
        client.say(channel.to_string(), "Usage: !setactivegames <redeem_name> <game1> [game2] [game3] ...".parse().unwrap()).await?;
        return Ok(());
    }

    let redeem_name = params[0];
    let games: Vec<String> = params[1..].iter().map(|&s| s.to_string()).collect();

    let mut manager = redeem_manager.write().await;
    let mut updated = false;

    {
        let mut handlers = manager.handlers_by_id.write().await;
        if let Some(settings) = handlers.values_mut().find(|s| s.title == redeem_name) {
            settings.active_games = games.clone();
            updated = true;
        }
    }

    if updated {
        manager.save_settings().await?;
        manager.update_twitch_redeems().await?;
        client.say(channel.to_string(), format!("Active games for '{}' set to: {}", redeem_name, games.join(", "))).await?;
    } else {
        client.say(channel.to_string(), format!("Redeem '{}' not found", redeem_name)).await?;
    }

    Ok(())
}