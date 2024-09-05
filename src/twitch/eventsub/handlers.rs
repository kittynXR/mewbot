use serde_json::Value;
use super::events;
use super::events::{channel_follow, channel_raid, channel_update, stream_online, stream_offline, channel_subscribe, channel_subscription_message, channel_point_redemption};
use super::events::{channel_bits, channel_subscription_gift, channel_subscription_end};
use super::events::ads; // New import
use std::sync::Arc;
use log::{debug, error};
use crate::twitch::manager::TwitchManager;

pub async fn handle_message(
    message: &str,
    twitch_manager: &Arc<TwitchManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!("Received EventSub message: {}", message);
    let parsed: Value = serde_json::from_str(message)?;

    let channel = twitch_manager.config.twitch_channel_to_join.as_ref()
        .ok_or("Twitch channel not set")?;

    if let Some(event_type) = parsed["metadata"]["subscription_type"].as_str() {
        match event_type {
            "channel.update" => channel_update::handle(&parsed, channel, twitch_manager).await?,
            "channel.follow" => channel_follow::handle(&parsed, channel, twitch_manager).await?,
            "channel.raid" => channel_raid::handle(&parsed, channel, twitch_manager).await?,
            "channel.shoutout.create" => events::shoutout::handle_shoutout_create(&parsed, channel, twitch_manager).await?,
            "channel.shoutout.receive" => events::shoutout::handle_shoutout_receive(&parsed, channel, twitch_manager).await?,
            "stream.online" => stream_online::handle(&parsed, channel, twitch_manager).await?,
            "stream.offline" => stream_offline::handle(&parsed,  channel, twitch_manager).await?,
            "channel.subscribe" => channel_subscribe::handle(&parsed, channel, twitch_manager).await?,
            "channel.subscription.message" => channel_subscription_message::handle(&parsed, channel, twitch_manager).await?,
            "channel.subscription.gift" => channel_subscription_gift::handle(&parsed, channel, twitch_manager).await?,
            "channel.subscription.end" => channel_subscription_end::handle(&parsed, channel, twitch_manager).await?,
            "channel.cheer" => channel_bits::handle(&parsed, channel, twitch_manager).await?,
            "channel.channel_points_custom_reward_redemption.add" => {
                debug!("Received new channel point redemption: {:?}", parsed["payload"]["event"]);
                channel_point_redemption::handle_new_redemption(&parsed["payload"]["event"], twitch_manager, channel).await?;
            },
            "channel.channel_points_custom_reward_redemption.update" => {
                debug!("Received channel point redemption update: {:?}", parsed["payload"]["event"]);
                channel_point_redemption::handle_redemption_update(&parsed["payload"]["event"]).await?;
            },
            "channel.ad_break.begin" => ads::handle_ad_break_begin(&parsed, channel, twitch_manager).await?, // New handler
            _ => error!("Unhandled event type: {}", event_type),
        }
    }

    Ok(())
}