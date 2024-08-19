// src/OBS/models.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OBSInstance {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub password: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OBSScene {
    pub name: String,
    pub items: Vec<OBSSceneItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OBSSceneItem {
    pub name: String,
    pub source_type: String,
    pub visible: bool,
}

// Add more data structures as needed for your OBS interactions