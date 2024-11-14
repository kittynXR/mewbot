// src/discord/announcements.rs
use serenity::builder::{CreateEmbed, CreateEmbedFooter, CreateMessage};
use serenity::model::prelude::*;
use serenity::http::Http;
use chrono::{DateTime, Utc};
use log::info;

pub async fn send_stream_announcement(
    http: &Http,
    channel_id: ChannelId,
    broadcaster_name: &str,
    started_at: &str,
    game_name: Option<&str>,
    title: Option<&str>,
    thumbnail_url: Option<&str>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Creating stream announcement for {}", broadcaster_name);

    let started_time = DateTime::parse_from_rfc3339(started_at)
        .map_err(|e| format!("Failed to parse start time: {}", e))?
        .with_timezone(&Utc);

    let mut embed = CreateEmbed::default()
        .color(0x9146FF) // Twitch purple
        .title(format!("🔴 {} is now live on Twitch!", broadcaster_name))
        .url(format!("https://twitch.tv/{}", broadcaster_name))
        .timestamp(started_time);

    if let Some(game) = game_name {
        embed = embed.field("Game", game, true);
    }

    if let Some(stream_title) = title {
        embed = embed.field("Title", stream_title, true);
    }

    if let Some(thumbnail) = thumbnail_url {
        embed = embed.thumbnail(thumbnail);
    }

    embed = embed.footer(CreateEmbedFooter::new("Come join the stream!"));

    channel_id.send_message(&http, CreateMessage::default()
        .content(format!("Hey @everyone, {} is live!", broadcaster_name))
        .embed(embed)
    ).await?;

    info!("Successfully sent stream announcement to Discord");
    Ok(())
}