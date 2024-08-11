use crate::twitch::redeems::{RedeemManager, RedemptionActionConfig, RedemptionActionType};
use crate::osc::models::{OSCConfig, OSCMessageType, OSCValue};
use crate::twitch::api::TwitchAPIClient;
use twitch_irc::message::PrivmsgMessage;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use crate::twitch::irc::TwitchBotClient;

pub async fn handle_add_redeem(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    api_client: &Arc<TwitchAPIClient>,
    redeem_manager: &Arc<RwLock<RedeemManager>>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
    params: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Join all parameters and then split them properly
    let full_command = params.join(" ");

    // Parse the title (which might be quoted)
    let (title, rest) = if full_command.starts_with('"') {
        match full_command[1..].split_once('"') {
            Some((title, rest)) => (title.to_string(), rest.trim_start()),
            None => {
                client.send_message(channel, "Invalid command format. Use: !add_redeem \"<title>\" <cost> <action_type> <cooldown> \"<prompt>\" [queued] [announce] [offline_chat_redeem] [game1] [game2] ...").await?;
                return Ok(());
            }
        }
    } else {
        match full_command.split_once(' ') {
            Some((title, rest)) => (title.to_string(), rest),
            None => {
                client.send_message(channel, "Invalid command format. Use: !add_redeem \"<title>\" <cost> <action_type> <cooldown> \"<prompt>\" [queued] [announce] [offline_chat_redeem] [game1] [game2] ...").await?;
                return Ok(());
            }
        }
    };

    // Parse the rest of the parameters
    let mut parts = rest.split_whitespace();

    let cost = match parts.next() {
        Some(c) => c.parse::<u32>().map_err(|_| "Invalid cost")?,
        None => {
            client.send_message(channel, "Missing cost parameter").await?;
            return Ok(());
        }
    };

    let action_type = match parts.next() {
        Some(a) => a,
        None => {
            client.send_message(channel, "Missing action type parameter").await?;
            return Ok(());
        }
    };

    let cooldown = match parts.next() {
        Some(c) => c.parse::<u32>().map_err(|_| "Invalid cooldown")?,
        None => {
            client.send_message(channel, "Missing cooldown parameter").await?;
            return Ok(());
        }
    };

    // Parse the prompt (which might be quoted)
    let prompt = if rest.trim_start().starts_with('"') {
        match rest.trim_start()[1..].split_once('"') {
            Some((prompt, rest)) => {
                parts = rest.trim_start().split_whitespace();
                prompt.to_string()
            }
            None => {
                client.send_message(channel, "Invalid prompt format. Prompt should be enclosed in quotes.").await?;
                return Ok(());
            }
        }
    } else {
        client.send_message(channel, "Missing prompt parameter. Prompt should be enclosed in quotes.").await?;
        return Ok(());
    };

    let queued = parts.next().map_or(false, |v| v == "true");
    let announce = parts.next().map_or(false, |v| v == "true");
    let offline_chat_redeem = parts.next().map_or(false, |v| v == "true");
    let auto_complete = parts.next().map_or(false, |v| v == "true");  // Add this line

    // Parse active games (optional)
    let active_games: Vec<String> = parts.map(|s| s.to_string()).collect();

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
            client.send_message(channel, "Invalid action type. Use 'ai', 'osc', or 'custom'.").await?;
            return Ok(());
        }
    };

    let mut redeem_manager = redeem_manager.write().await;
    match redeem_manager.add_redeem_at_runtime(
        title.clone(),
        cost,
        action_config,
        None,
        cooldown,
        prompt,
        active_games,
        offline_chat_redeem,
        Some(OSCConfig {
            uses_osc: false,
            osc_endpoint: String::new(),
            osc_type: OSCMessageType::Boolean,
            osc_value: OSCValue::Boolean(false),
            default_value: OSCValue::Integer(0),
            execution_duration: Some(Duration::from_secs(3)),
            send_chat_message: false,
        }),
        auto_complete,  // Add this line
    ).await {
        Ok(_) => {
            client.send_message(channel, &format!("New redeem '{}' added successfully!", title)).await?;
        }
        Err(e) => {
            client.send_message(channel, &format!("Failed to add new redeem: {}", e)).await?;
        }
    }

    Ok(())
}
