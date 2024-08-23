use crate::twitch::redeems::models::{Redemption, RedemptionResult};
use crate::twitch::api::TwitchAPIClient;
use rand::{SeedableRng, Rng};
use rand::rngs::SmallRng;
use std::sync::Arc;
use log::{debug, error};
use crate::twitch::irc::client::TwitchIRCClientType;
use crate::twitch::irc::TwitchBotClient;
use crate::twitch::redeems::RedeemManager;

pub async fn handle_custom_action(
    redemption: &Redemption,
    action_name: &str,
    api_client: &TwitchAPIClient,
    irc_client: &Arc<TwitchBotClient>,
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
    irc_client: &Arc<TwitchBotClient>,
    channel: &str,
    redeem_manager: &RedeemManager
) -> RedemptionResult {
    debug!("Executing CoinGameAction for redemption: {:?}", redemption);

    let mut rng = SmallRng::from_entropy();
    let multiplier = rng.gen_range(1.5..=2.5);

    let mut state = redeem_manager.coin_game_state.write().await;
    let current_price = state.current_price;
    let new_price = (current_price as f64 * multiplier).round() as u32;

    // Get the cooldown and prompt from the settings
    let (cooldown, mut prompt) = {
        let handlers = redeem_manager.handlers_by_name.read().await;
        handlers.get(&redemption.reward_id)
            .map(|settings| (settings.cooldown, settings.prompt.clone()))
            .unwrap_or((0, "Enter the coin game!".to_string()))
    };

    // Generate AI message
    let ai_prompt = format!("Create a short, fun message (max 50 characters) about {} entering the coin game.", redemption.user_name);
    let ai_message = match redeem_manager.ai_client.as_ref().unwrap().generate_response_without_history(&ai_prompt).await {
        Ok(message) => message,
        Err(_) => "has entered the coin game!".to_string(),  // Fallback message
    };

    // Update the prompt with the user's name and AI-generated message
    prompt = format!("{} {} New is {} pawmarks!", redemption.user_name, ai_message, new_price);


    if let Some(previous_redemption) = state.last_redemption.take() {
        // Refund the previous redemption
        if let Err(e) = api_client.refund_channel_points(&previous_redemption.reward_id, &previous_redemption.id).await {
            error!("Failed to refund previous coin game redemption: {}", e);
        } else {
            let refund_message = format!(
                "{} is cute!",
                previous_redemption.user_name
            );
            if let Err(e) = irc_client.send_message(channel, &refund_message).await {
                error!("Failed to send refund message to chat: {}", e);
            }
        }
    }

    // Update the reward cost and prompt
    if let Err(e) = api_client.update_custom_reward(&redemption.reward_id, &redemption.reward_title, new_price, true, cooldown, &prompt).await {
        eprintln!("Failed to update reward: {}", e);
        return RedemptionResult {
            success: false,
            message: Some("Failed to update reward".to_string()),
            queue_number: redemption.queue_number,
        };
    }

    // Send a message to the chat
    let chat_message = format!(
        "{} spent {} pawmarks! The new price is {} pawmarks! Who's next?",
        redemption.user_name, current_price, new_price
    );
    if let Err(e) = irc_client.send_message(channel, &chat_message).await {
        error!("Failed to send message to chat: {}", e);
    }

    // Update the state
    state.current_price = new_price;
    state.last_redemption = Some(redemption.clone());

    RedemptionResult {
        success: true,
        message: Some(format!("Coin game! {}  {}", new_price, prompt)),
        queue_number: redemption.queue_number,
    }
}