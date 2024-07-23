use crate::twitch::redeems::models::{Redemption, RedemptionResult};
use crate::twitch::api::TwitchAPIClient;
use rand::{SeedableRng, Rng};
use rand::rngs::SmallRng;
use std::sync::Arc;
use crate::twitch::irc::client::TwitchIRCClientType;
use crate::twitch::redeems::RedeemManager;

pub async fn handle_custom_action(
    redemption: &Redemption,
    action_name: &str,
    api_client: &TwitchAPIClient,
    irc_client: &Arc<TwitchIRCClientType>,
    channel: &str,
    redeem_manager: &RedeemManager
) -> RedemptionResult {
    match action_name {
        "coin game" => handle_coin_game(redemption, api_client, irc_client, channel, redeem_manager).await,
        _ => RedemptionResult {
            success: false,
            message: Some(format!("Unknown custom action: {}", action_name)),
            queue_number: redemption.queue_number,
        },
    }
}

pub async fn handle_coin_game(
    redemption: &Redemption,
    api_client: &TwitchAPIClient,
    irc_client: &Arc<TwitchIRCClientType>,
    channel: &str,
    redeem_manager: &RedeemManager
) -> RedemptionResult {
    println!("Executing CoinGameAction for redemption: {:?}", redemption);

    let mut rng = SmallRng::from_entropy();
    let multiplier = rng.gen_range(1.5..=2.5);

    let mut state = redeem_manager.coin_game_state.write().await;
    let current_price = state.current_price;
    let new_price = (current_price as f64 * multiplier).round() as u32;

    if let Some(previous_redemption) = state.last_redemption.take() {
        // Refund the previous redemption
        if let Err(e) = api_client.refund_channel_points(&previous_redemption.reward_id, &previous_redemption.id).await {
            eprintln!("Failed to refund previous coin game redemption: {}", e);
        } else {
            let refund_message = format!(
                "{} has been refunded {} points for the previous coin game!",
                previous_redemption.user_name, current_price
            );
            if let Err(e) = irc_client.say(channel.to_string(), refund_message).await {
                eprintln!("Failed to send refund message to chat: {}", e);
            }
        }
    }

    // Update the reward cost
    if let Err(e) = api_client.update_custom_reward(&redemption.reward_id, &redemption.reward_title, new_price, true).await {
        eprintln!("Failed to update reward cost: {}", e);
        return RedemptionResult {
            success: false,
            message: Some("Failed to update reward cost".to_string()),
            queue_number: redemption.queue_number,
        };
    }

    // Send a message to the chat
    let chat_message = format!(
        "{} spent {} points on the coin game! The new price is {} points! Who's next?",
        redemption.user_name, current_price, new_price
    );
    if let Err(e) = irc_client.say(channel.to_string(), chat_message).await {
        eprintln!("Failed to send message to chat: {}", e);
    }

    // Update the state
    state.current_price = new_price;
    state.last_redemption = Some(redemption.clone());

    RedemptionResult {
        success: true,
        message: Some(format!("Coin game! The new price is {} points", new_price)),
        queue_number: redemption.queue_number,
    }
}