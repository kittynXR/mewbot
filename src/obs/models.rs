// src/obs/models.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsInstance {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsScene {
    pub name: String,
    pub items: Vec<ObsSceneItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObsSceneItem {
    pub name: String,
    pub source_type: String,
    pub visible: bool,
}

// Add more data structures as needed for your OBS interactions