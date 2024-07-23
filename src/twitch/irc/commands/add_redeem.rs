use crate::twitch::redeems::{RedeemManager, RedemptionActionConfig, RedemptionActionType};
use crate::twitch::api::TwitchAPIClient;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use tokio::sync::RwLock;

pub async fn handle_add_redeem(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    api_client: &Arc<TwitchAPIClient>,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Join all parameters and then split them properly
    let full_command = params.join(" ");

    // Parse the title (which might be quoted)
    let (title, rest) = if full_command.starts_with('"') {
        match full_command[1..].split_once('"') {
            Some((title, rest)) => (title.to_string(), rest.trim_start()),
            None => {
                client.say(channel.to_string(), "Invalid command format. Use: !add_redeem \"<title>\" <cost> <action_type> [queued] [announce]".to_string()).await?;
                return Ok(());
            }
        }
    } else {
        match full_command.split_once(' ') {
            Some((title, rest)) => (title.to_string(), rest),
            None => {
                client.say(channel.to_string(), "Invalid command format. Use: !add_redeem \"<title>\" <cost> <action_type> [queued] [announce]".to_string()).await?;
                return Ok(());
            }
        }
    };

    // Parse the rest of the parameters
    let mut parts = rest.split_whitespace();

    let cost = match parts.next() {
        Some(c) => c.parse::<u32>().map_err(|_| "Invalid cost")?,
        None => {
            client.say(channel.to_string(), "Missing cost parameter".to_string()).await?;
            return Ok(());
        }
    };

    let action_type = match parts.next() {
        Some(a) => a,
        None => {
            client.say(channel.to_string(), "Missing action type parameter".to_string()).await?;
            return Ok(());
        }
    };

    let queued = parts.next().map_or(false, |v| v == "true");
    let announce = parts.next().map_or(false, |v| v == "true");

    let action_config = match action_type {
        "ai" => RedemptionActionConfig {
            action: RedemptionActionType::AIResponse,
            queued,
            announce_in_chat: announce,
            requires_manual_completion: false,
        },
        "osc" => RedemptionActionConfig {
            action: RedemptionActionType::OSCMessage,
            queued,
            announce_in_chat: announce,
            requires_manual_completion: false,
        },
        "custom" => RedemptionActionConfig {
            action: RedemptionActionType::Custom(title.clone()),
            queued,
            announce_in_chat: announce,
            requires_manual_completion: false,
        },
        _ => {
            client.say(channel.to_string(), "Invalid action type. Use 'ai', 'osc', or 'custom'.".to_string()).await?;
            return Ok(());
        }
    };

    let mut redeem_manager = redeem_manager.write().await;
    match redeem_manager.add_redeem_at_runtime(title.clone(), cost, action_config, None).await {
        Ok(_) => {
            client.say(channel.to_string(), format!("New redeem '{}' added successfully!", title)).await?;
        }
        Err(e) => {
            client.say(channel.to_string(), format!("Failed to add new redeem: {}", e)).await?;
        }
    }

    Ok(())
}