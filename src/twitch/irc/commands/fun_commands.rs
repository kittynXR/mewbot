use crate::twitch::irc::TwitchBotClient;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use crate::ai::AIClient;
use twitch_irc::message::PrivmsgMessage;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::twitch::TwitchManager;
use chrono::{Utc, Datelike, NaiveDate};

pub async fn handle_isitfriday(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
    ai_client: &Option<Arc<AIClient>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let is_friday = Utc::now().weekday().num_days_from_monday() == 4;
    let friday_message = generate_friday_message(ai_client, is_friday).await;

    twitch_manager.send_message_as_bot(channel, &friday_message).await?;

    Ok(())
}

pub async fn handle_xmas(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
    ai_client: &Option<Arc<AIClient>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let days_until_christmas = calculate_days_until_christmas();
    let xmas_message = generate_xmas_message(ai_client, days_until_christmas).await;

    twitch_manager.send_message_as_bot(channel, &xmas_message).await?;

    Ok(())
}

async fn generate_friday_message(ai_client: &Option<Arc<AIClient>>, is_friday: bool) -> String {
    if let Some(ai) = ai_client {
        let prompt = if is_friday {
            "Generate a joyful, exuberant celebratory message about it being Friday. Keep it short and fun!"
        } else {
            "Generate a meek, slightly disappointed message about it not being Friday yet. Keep it short and a bit humorous!"
        };

        match ai.generate_response_without_history(prompt).await {
            Ok(response) => response.trim().to_string(),
            Err(e) => {
                eprintln!("Error generating AI response: {:?}", e);
                default_friday_message(is_friday)
            }
        }
    } else {
        default_friday_message(is_friday)
    }
}

fn default_friday_message(is_friday: bool) -> String {
    if is_friday {
        "It's Friday! Time to celebrate! 🎉".to_string()
    } else {
        "Not Friday yet... 😔 But we're getting there!".to_string()
    }
}

fn calculate_days_until_christmas() -> i64 {
    let today = Utc::now().date_naive();
    let current_year = today.year();
    let christmas = NaiveDate::from_ymd_opt(current_year, 12, 25).unwrap();

    let days = christmas.signed_duration_since(today).num_days();
    if days < 0 {
        // If Christmas has passed this year, calculate for next year
        let next_christmas = NaiveDate::from_ymd_opt(current_year + 1, 12, 25).unwrap();
        next_christmas.signed_duration_since(today).num_days()
    } else {
        days
    }
}

async fn generate_xmas_message(ai_client: &Option<Arc<AIClient>>, days_until_christmas: i64) -> String {
    if let Some(ai) = ai_client {
        let prompt = format!(
            "Generate a very short, fun message about there being {} days until Christmas. Keep it under 100 characters!",
            days_until_christmas
        );

        match ai.generate_response_without_history(&prompt).await {
            Ok(response) => response.trim().to_string(),
            Err(e) => {
                eprintln!("Error generating AI response: {:?}", e);
                default_xmas_message(days_until_christmas)
            }
        }
    } else {
        default_xmas_message(days_until_christmas)
    }
}

fn default_xmas_message(days_until_christmas: i64) -> String {
    format!("{} days until Christmas! 🎄🎅", days_until_christmas)
}