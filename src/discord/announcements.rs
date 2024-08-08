use serenity::prelude::*;
use serenity::model::prelude::*;

pub async fn send_stream_announcement(ctx: &Context, channel_id: ChannelId, status: &str, title: &str) {
    let _ = channel_id.say(&ctx.http, format!("Stream {}: {}", status, title)).await;
}