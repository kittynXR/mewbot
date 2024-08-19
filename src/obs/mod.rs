// src/obs/mod.rs

mod models;
mod websocket;

pub use models::*;
pub use websocket::{ObsWebSocketClient, ObsManager};

// If you add more files in the future, you can include them here
// mod additional_feature;
// pub use additional_feature::SomeFeature;