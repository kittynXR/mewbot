// src/twitch/mod.rs
pub mod utils;
pub mod api;
pub mod irc;
pub mod eventsub;
pub mod pubsub;
pub mod redeems;
pub mod roles;
pub mod role_cache;

pub use api::TwitchAPIClient;
pub use irc::client::TwitchIRCManager;