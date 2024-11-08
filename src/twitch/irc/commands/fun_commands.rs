use crate::twitch::irc::command_system::{Command, CommandContext};
use crate::twitch::roles::UserRole;
use crate::ai::AIClient;
use std::sync::Arc;
use chrono::{Utc, Datelike, NaiveDate, Weekday};

pub struct IsItFridayCommand;
pub struct XmasCommand;

#[async_trait::async_trait]
impl Command for IsItFridayCommand {
    fn name(&self) -> &'static str {
        "!isitfriday"
    }

    fn description(&self) -> &'static str {
        "Check if it's Friday and get a fun message"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let is_friday = Utc::now().weekday().num_days_from_monday() == 4;
        let friday_message = generate_friday_message(&ctx.ai_client, is_friday).await;

        ctx.twitch_manager.send_message_as_bot(&ctx.channel, &friday_message).await?;

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Viewer
    }
}

#[async_trait::async_trait]
impl Command for XmasCommand {
    fn name(&self) -> &'static str {
        "!xmas"
    }

    fn description(&self) -> &'static str {
        "Find out how many days until Christmas"
    }

    async fn execute(&self, ctx: &CommandContext, _args: Vec<String>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let days_until_christmas = calculate_days_until_christmas();
        let xmas_message = generate_xmas_message(&ctx.ai_client, days_until_christmas).await;

        ctx.twitch_manager.send_message_as_bot(&ctx.channel, &xmas_message).await?;

        Ok(())
    }

    fn required_role(&self) -> UserRole {
        UserRole::Viewer
    }
}

async fn generate_friday_message(ai_client: &Option<Arc<AIClient>>, is_friday: bool) -> String {
    let current_day = Utc::now().weekday();

    if let Some(ai) = ai_client {
        let prompt = if is_friday {
            "Generate a joyful, exuberant celebratory message about it being Friday today. Keep it short and fun!"
        } else {
            &*format!(
                "Generate a meek, slightly disappointed message about it being {} and not Friday yet. Keep it short and a bit humorous!",
                current_day
            )
        };

        match ai.generate_response_without_history(&prompt).await {
            Ok(response) => response.trim().to_string(),
            Err(e) => {
                eprintln!("Error generating AI response: {:?}", e);
                default_friday_message(is_friday, current_day)
            }
        }
    } else {
        default_friday_message(is_friday, current_day)
    }
}

fn default_friday_message(is_friday: bool, current_day: Weekday) -> String {
    if is_friday {
        "It's Friday! Time to celebrate! 🎉".to_string()
    } else {
        format!("It's {}... Not Friday yet... 😔 But we're getting there!", current_day)
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