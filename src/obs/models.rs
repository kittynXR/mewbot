use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use log::{error, info};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tungstenite::Message;
use async_trait::async_trait;
use crate::web_ui::websocket::{DashboardState, WebSocketMessage};
use serde_json::{json, Value};
use crate::obs::websocket::ConnectionState;

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
    pub response_channels: Arc<RwLock<HashMap<String, oneshot::Sender<serde_json::Value>>>>,
    pub(crate) connection_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    pub(crate) should_reconnect: Arc<AtomicBool>,
}


pub struct OBSClientState {
    pub connection: Option<mpsc::UnboundedSender<Message>>,
    pub connection_state: ConnectionState,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct OBSInstanceState {
    pub name: String,
    pub scenes: Vec<String>,
    pub current_scene: String,
    pub sources: HashMap<String, Vec<OBSSceneItem>>,
}

pub struct OBSManager {
    clients: Arc<RwLock<HashMap<String, OBSWebSocketClient>>>,
    ws_sender: mpsc::UnboundedSender<WebSocketMessage>,
}

impl OBSManager {
    pub fn new(ws_sender: mpsc::UnboundedSender<WebSocketMessage>) -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
            ws_sender,
        }
    }

    pub async fn handle_message(&self, message: WebSocketMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match message.action.as_str() {
            "get_info" => self.get_info().await,
            "change_scene" => {
                let instance_name = message.data["instance_name"].as_str().ok_or("Missing instance_name")?;
                let scene_name = message.data["scene_name"].as_str().ok_or("Missing scene_name")?;
                self.change_scene(instance_name, scene_name).await
            },
            "toggle_source" => {
                let instance_name = message.data["instance_name"].as_str().ok_or("Missing instance_name")?;
                let scene_name = message.data["scene_name"].as_str().ok_or("Missing scene_name")?;
                let source_name = message.data["source_name"].as_str().ok_or("Missing source_name")?;
                let enabled = message.data["enabled"].as_bool().ok_or("Missing enabled status")?;
                self.toggle_source(instance_name, scene_name, source_name, enabled).await
            },
            "refresh_source" => {
                let instance_name = message.data["instance_name"].as_str().ok_or("Missing instance_name")?;
                let source_name = message.data["source_name"].as_str().ok_or("Missing source_name")?;
                self.refresh_source(instance_name, source_name).await
            },
            _ => Err("Unknown OBS action".into()),
        }
    }

    async fn get_info(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let instances = self.get_instances().await;
        self.send_update(json!({
            "instances": instances,
            "status": true,
        })).await
    }

    async fn change_scene(&self, instance_name: &str, scene_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(instance_name) {
            client.set_current_scene(scene_name).await?;
            drop(clients);
            self.get_info().await?;
        } else {
            return Err(format!("OBS instance not found: {}", instance_name).into());
        }
        Ok(())
    }

    async fn toggle_source(&self, instance_name: &str, scene_name: &str, source_name: &str, enabled: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(instance_name) {
            client.set_scene_item_enabled(scene_name, source_name, enabled).await?;
            drop(clients);
            self.get_info().await?;
        } else {
            return Err(format!("OBS instance not found: {}", instance_name).into());
        }
        Ok(())
    }

    async fn refresh_source(&self, instance_name: &str, source_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.get_mut(instance_name) {
            client.refresh_browser_source(source_name).await?;
            // Optionally get and send updated info
            // self.get_info().await?;
        } else {
            return Err(format!("OBS instance not found: {}", instance_name).into());
        }
        Ok(())
    }

    async fn get_instances(&self) -> Vec<OBSInstanceState> {
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

    async fn send_update(&self, update: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let message = WebSocketMessage {
            module: "obs".to_string(),
            action: "update".to_string(),
            data: update,
        };
        self.ws_sender.send(message);
        Ok(())
    }

    pub async fn add_instance(&self, name: String, instance: OBSInstance) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = OBSWebSocketClient::new(instance);
        self.clients.write().await.insert(name.clone(), client.clone());
        if let Err(e) = client.connect().await {
            error!("Failed to start connection manager for OBS instance {}: {}", name, e);
            self.clients.write().await.remove(&name);
            return Err(e);
        }
        self.get_info().await?;
        Ok(())
    }

    pub async fn remove_instance(&self, name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut clients = self.clients.write().await;
        if let Some(client) = clients.remove(name) {
            client.disconnect().await?;
        }
        self.get_info().await?;
        Ok(())
    }
}