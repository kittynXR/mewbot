use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, RwLock};
use tungstenite::Message;
use async_trait::async_trait;
use crate::web_ui::websocket::DashboardState;

#[async_trait]
pub trait OBSStateUpdate: Send + Sync {
    async fn update_obs_state(&self, instances: Vec<OBSInstanceState>);
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OBSInstance {
    pub name: String,
    pub address: String,
    pub port: u16,
    pub auth_required: bool,
    pub password: Option<String>,
    pub use_ssl: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OBSScene {
    pub name: String,
    pub items: Vec<OBSSceneItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OBSSceneItem {
    pub name: String,
    pub source_type: String,
    pub visible: bool,
}

// Remove the #[derive(Clone)] attribute
pub struct OBSWebSocketClient {
    pub instance: OBSInstance,
    pub state: Arc<RwLock<OBSClientState>>,
    pub response_channels: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<serde_json::Value>>>>,
}

pub struct OBSClientState {
    pub connection: Option<mpsc::UnboundedSender<Message>>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct OBSInstanceState {
    pub name: String,
    pub scenes: Vec<String>,
    pub current_scene: String,
    pub sources: HashMap<String, Vec<OBSSceneItem>>,
}

pub struct OBSManager {
    pub clients: Arc<RwLock<HashMap<String, OBSWebSocketClient>>>,
    state_updater: Arc<RwLock<DashboardState>>,
}

impl OBSManager {
    pub fn new(state_updater: Arc<RwLock<DashboardState>>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            state_updater,
        }
    }

    pub async fn update_dashboard_state(&self) {
        let instances = self.get_instances().await;
        let mut dashboard_state = self.state_updater.write().await;
        dashboard_state.obs_instances = instances;
    }

    pub async fn add_instance(&self, name: String, instance: OBSInstance) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = OBSWebSocketClient::new(instance);

        info!("Attempting to add OBS instance: {}", name);
        match tokio::time::timeout(Duration::from_secs(35), client.connect()).await {
            Ok(result) => {
                match result {
                    Ok(_) => {
                        self.clients.write().await.insert(name.clone(), client);
                        info!("Successfully added OBS instance: {}", name);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Failed to connect to OBS instance {}: {}. This instance will be unavailable.", name, e);
                        Ok(()) // We return Ok to allow the program to continue with other instances
                    }
                }
            }
            Err(_) => {
                error!("Timeout occurred while connecting to OBS instance {}. This instance will be unavailable.", name);
                Ok(()) // We return Ok to allow the program to continue with other instances
            }
        }
    }
    pub async fn get_instances(&self) -> Vec<OBSInstanceState> {
        let clients = self.clients.read().await;
        let mut instances = Vec::new();

        for (name, client) in clients.iter() {
            match client.get_instance_state().await {
                Ok(state) => instances.push(state),
                Err(e) => error!("Failed to get state for OBS instance {}: {}", name, e),
            }
        }
        instances
    }
}