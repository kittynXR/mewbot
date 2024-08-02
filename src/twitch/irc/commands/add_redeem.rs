use crate::twitch::redeems::{RedeemManager, RedemptionActionConfig, RedemptionActionType};
use crate::osc::models::{OSCConfig, OSCMessageType, OSCValue};
use crate::twitch::api::TwitchAPIClient;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use std::time::Duration;
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
                client.say(channel.to_string(), "Invalid command format. Use: !add_redeem \"<title>\" <cost> <action_type> <cooldown> \"<prompt>\" [queued] [announce] [offline_chat_redeem] [game1] [game2] ...".to_string()).await?;
                return Ok(());
            }
        }
    } else {
        match full_command.split_once(' ') {
            Some((title, rest)) => (title.to_string(), rest),
            None => {
                client.say(channel.to_string(), "Invalid command format. Use: !add_redeem \"<title>\" <cost> <action_type> <cooldown> \"<prompt>\" [queued] [announce] [offline_chat_redeem] [game1] [game2] ...".to_string()).await?;
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

    let cooldown = match parts.next() {
        Some(c) => c.parse::<u32>().map_err(|_| "Invalid cooldown")?,
        None => {
            client.say(channel.to_string(), "Missing cooldown parameter".to_string()).await?;
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
                client.say(channel.to_string(), "Invalid prompt format. Prompt should be enclosed in quotes.".to_string()).await?;
                return Ok(());
            }
        }
    } else {
        client.say(channel.to_string(), "Missing prompt parameter. Prompt should be enclosed in quotes.".to_string()).await?;
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
            client.say(channel.to_string(), "Invalid action type. Use 'ai', 'osc', or 'custom'.".to_string()).await?;
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
            client.say(channel.to_string(), format!("New redeem '{}' added successfully!", title)).await?;
        }
        Err(e) => {
            client.say(channel.to_string(), format!("Failed to add new redeem: {}", e)).await?;
        }
    }

    Ok(())
}
