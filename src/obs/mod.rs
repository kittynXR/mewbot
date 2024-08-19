// src/obs/mod.rs

pub(crate) mod models;
pub(crate) mod websocket;

pub use models::*;
pub use websocket::{OBSWebSocketClient, OBSManager};


// If you add more files in the future, you can include them here
// mod additional_feature;
// pub use additional_feature::SomeFeature;