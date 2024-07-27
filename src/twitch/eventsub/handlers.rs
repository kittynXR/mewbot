use serde_json::Value;
use super::client::TwitchEventSubClient;
use super::events;
use super::events::{channel_follow, channel_raid, channel_update, stream_online, stream_offline, channel_subscribe, channel_subscription_message};
use super::events::{channel_subscription_gift, channel_subscription_end};
use twitch_irc::TwitchIRCClient as ExternalTwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use std::sync::Arc;
use crate::twitch::api::TwitchAPIClient;

pub async fn handle_message(
    message: &str,
    irc_client: &Arc<ExternalTwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &str,
    api_client: &Arc<TwitchAPIClient>,
    eventsub_client: &TwitchEventSubClient,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Received EventSub message: {}", message);
    let parsed: Value = serde_json::from_str(message)?;

    if let Some(event_type) = parsed["metadata"]["subscription_type"].as_str() {
        match event_type {
            "channel.update" => channel_update::handle(&parsed, irc_client, channel).await?,
            "channel.follow" => channel_follow::handle(&parsed, irc_client, channel).await?,
            "channel.raid" => channel_raid::handle(&parsed, irc_client, channel, api_client).await?,
            "channel.shoutout.create" => events::shoutout::handle_shoutout_create(&parsed, irc_client, channel).await?,
            "channel.shoutout.receive" => events::shoutout::handle_shoutout_receive(&parsed, irc_client, channel).await?,
            "stream.online" => stream_online::handle(&parsed, irc_client, channel, &eventsub_client.redeem_manager).await?,
            "stream.offline" => stream_offline::handle(&parsed, irc_client, channel, &eventsub_client.redeem_manager).await?,
            "channel.subscribe" => channel_subscribe::handle(&parsed, irc_client, channel).await?,
            "channel.subscription.message" => channel_subscription_message::handle(&parsed, irc_client, channel).await?,
            "channel.subscription.gift" => channel_subscription_gift::handle(&parsed, irc_client, channel).await?,
            "channel.subscription.end" => channel_subscription_end::handle(&parsed, irc_client, channel).await?,
            "channel.channel_points_custom_reward_redemption.add" => {
                println!("Received new channel point redemption: {:?}", parsed["payload"]["event"]);
                eventsub_client.handle_new_channel_point_redemption(&parsed["payload"]["event"]).await?;
            },
            "channel.channel_points_custom_reward_redemption.update" => {
                println!("Received channel point redemption update: {:?}", parsed["payload"]["event"]);
                eventsub_client.handle_channel_point_redemption_update(&parsed["payload"]["event"]).await?;
            },
            _ => println!("Unhandled event type: {}", event_type),
        }
    }

    Ok(())
}