use crate::twitch::irc::TwitchBotClient;
use crate::twitch::api::TwitchAPIClient;
use crate::twitch::api::requests::announcement::send_announcement;
use crate::storage::StorageClient;
use crate::discord::UserLinks;
use crate::ai::AIClient;
use twitch_irc::message::PrivmsgMessage;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::twitch::TwitchManager;

pub async fn handle_discord(
    msg: &PrivmsgMessage,
    client: &Arc<TwitchBotClient>,
    channel: &str,
    twitch_manager: &Arc<TwitchManager>,
    _storage: &Arc<RwLock<StorageClient>>,
    _user_links: &Arc<UserLinks>,
    ai_client: &Option<Arc<AIClient>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let irc_manager = client.get_manager();
    let discord_link = irc_manager.get_discord_link().await;

    // Generate a custom greeting using AI
    let discord_message = generate_discord_message(ai_client, &discord_link).await;

    // Send a message in the chat
    // client.send_message(channel, &discord_message).await?;

    let api_client = twitch_manager.get_api_client();
    // Send an announcement
    let broadcaster_id = api_client.get_broadcaster_id().await?;
    let bot_id = api_client.get_bot_id().await?;

    send_announcement(api_client, &broadcaster_id, &broadcaster_id, &discord_message, Some("primary")).await?;

    Ok(())
}

async fn generate_discord_message(ai_client: &Option<Arc<AIClient>>, discord_link: &str) -> String {
    if let Some(ai) = ai_client {
        let prompt = format!(
            "Generate a friendly and inviting message to encourage Twitch viewers to join our Discord community. \
            Feel free to mention: VR, technology, cute & funny anime, catgirls, foxgirls, \
            catboys, foxboys, 3D art or living in the matrix. \
            Don't use the word viewers.  If anything, say chatters or everyone. Good vibes. Good vibes. \
            The message should be brief (1-2 sentences) and include the following Discord link: {}. \
            Make sure the tone is casual and welcoming.",
            discord_link
        );

        match ai.generate_response_without_history(&prompt).await {
            Ok(response) => {
                // Add spaces around the discord_link within the response
                let cleaned_response = response.replace(discord_link, &format!(" {} ", discord_link));

                // Trim any leading or trailing whitespace
                cleaned_response.trim().to_string()
            }
            Err(e) => {
                eprintln!("Error generating AI response: {:?}", e);
                default_discord_message(discord_link)
            }
        }
    } else {
        default_discord_message(discord_link)
    }
}

fn default_discord_message(discord_link: &str) -> String {
    format!("Join our Discord community! {} ", discord_link)
}