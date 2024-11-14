// src/discord/mod.rs
mod client;
mod events;
mod commands;
mod link;
mod roles;
pub mod announcements;
pub use client::DiscordClient;
pub use link::UserLinks;