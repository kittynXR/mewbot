use crate::twitch::manager::TwitchManager;
use crate::twitch::models::{Redemption, RedemptionStatus};
use serde_json::Value;
use std::sync::Arc;
use log::{debug, error};



pub async fn handle_new_redemption(
    event: &Value,
    twitch_manager: &Arc<TwitchManager>,
    channel: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let redemption = parse_redemption(event);

    debug!("Processing new redemption: {:?}", redemption);

    let redeem_manager = twitch_manager.get_redeem_manager();
    let redeem_manager = redeem_manager.read().await;

    if let Some(redeem_manager) = redeem_manager.as_ref() {
        let result = redeem_manager.handle_redemption(&redemption).await;

        if result.success {
            debug!("Redemption handled successfully: {:?}", result);
            if let Some(ref message) = result.message {
                twitch_manager.send_message_as_bot(channel, &message).await?;
            }

            // Check if the redeem should be auto-completed
            let redeems = redeem_manager.registry.get_all().await;
            if let Some(redeem_info) = redeems.iter().find(|r| r.title == redemption.reward_title) {
                if redeem_info.auto_complete {
                    debug!("Auto-completing redemption: {}", redemption.reward_title);
                    twitch_manager.api_client.complete_channel_points(
                        &redemption.broadcaster_id,
                        &redemption.reward_id,
                        &redemption.id,
                    ).await?;
                }
            }
        } else {
            error!("Failed to handle redemption: {:?}", result);
        }
    } else {
        return Err("RedeemManager is not initialized".into());
    }

    Ok(())
}

pub async fn handle_redemption_update(
    event: &Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!("Received channel point redemption update: {:?}", event);

    let redemption = parse_redemption(event);
    let status: RedemptionStatus = redemption.status.clone();

    match status {
        RedemptionStatus::Unfulfilled => {
            debug!("Redemption is still unfulfilled: {:?}", redemption);
        },
        RedemptionStatus::Fulfilled => {
            debug!("Redemption has been fulfilled: {:?}", redemption);
        },
        RedemptionStatus::Canceled => {
            debug!("Redemption canceled: {:?}", redemption);
            // You might want to handle cancellation in your RedeemManager
            // twitch_manager.redeem_manager.write().await.cancel_redemption(&redemption.id).await?;
        },
    }

    Ok(())
}

fn parse_redemption(event: &Value) -> Redemption {
    Redemption {
        id: event["id"].as_str().unwrap_or("").to_string(),
        broadcaster_id: event["broadcaster_user_id"].as_str().unwrap_or("").to_string(),
        user_id: event["user_id"].as_str().unwrap_or("").to_string(),
        user_name: event["user_login"].as_str().unwrap_or("").to_string(),
        reward_id: event["reward"]["id"].as_str().unwrap_or("").to_string(),
        reward_title: event["reward"]["title"].as_str().unwrap_or("").to_string(),
        user_input: event["user_input"].as_str().map(|s| s.to_string()),
        status: event["status"].as_str().unwrap_or("unfulfilled").into(),
    }
}