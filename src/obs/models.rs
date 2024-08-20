// src/OBS/models.rs

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tungstenite::Message;

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

#[derive(Clone)]
pub struct OBSWebSocketClient {
    pub(crate) instance: OBSInstance,
    pub(crate) state: Arc<RwLock<OBSClientState>>,
}

pub struct OBSClientState {
    pub(crate) connection: Option<mpsc::UnboundedSender<Message>>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct OBSInstanceState {
    pub name: String,
    pub scenes: Vec<String>,
    pub current_scene: String,
    pub sources: HashMap<String, Vec<OBSSceneItem>>,
}

pub struct OBSManager {
    pub(crate) clients: Arc<RwLock<HashMap<String, OBSWebSocketClient>>>,
}