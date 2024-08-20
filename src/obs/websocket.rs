// src/OBS/websocket.rs

use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::protocol::Message,
    tungstenite::client::IntoClientRequest,
    tungstenite::handshake::client::generate_key
};
use tokio_tungstenite::tungstenite::http::Uri;
use serde_json::{json, Value};
use crate::obs::models::{OBSInstance, OBSScene};
use crate::obs::{OBSClientState, OBSInstanceState, OBSManager, OBSSceneItem, OBSWebSocketClient};

impl OBSWebSocketClient {
    pub fn new(instance: OBSInstance) -> Self {
        Self {
            instance,
            state: Arc::new(RwLock::new(OBSClientState {
                connection: None,
            })),
        }
    }

    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("ws://{}:{}", self.instance.address, self.instance.port);
        let uri: Uri = url.parse()?;
        let mut request = uri.into_client_request()?;

        // Set the WebSocket key
        let key = generate_key();
        request.headers_mut().insert(
            "Sec-WebSocket-Key",
            key.parse().expect("Failed to parse WebSocket key"),
        );

        let (ws_stream, _) = connect_async_with_config(request, None, false).await?;
        let (mut write, read) = ws_stream.split();

        let (tx, mut rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                if write.send(message).await.is_err() {
                    break;
                }
            }
        });

        let mut state = self.state.write().await;
        state.connection = Some(tx.clone());

        // Handle incoming messages
        tokio::spawn(async move {
            let mut read = read;
            while let Some(message) = read.next().await {
                if let Ok(message) = message {
                    if let Message::Text(text) = message {
                        println!("Received message: {}", text);
                        // Handle the message here
                    }
                }
            }
        });

        // Authenticate if a password is provided
        if let Some(password) = &self.instance.password {
            self.authenticate(password).await?;
        }

        Ok(())
    }

    async fn authenticate(&self, password: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let auth_payload = json!({
            "op": 6,
            "d": {
                "rpcVersion": 1,
                "authentication": password,
            }
        });

        self.send_message(auth_payload).await?;

        // Here you would typically wait for a response to confirm authentication
        // For simplicity, we're just assuming it succeeded

        Ok(())
    }

    async fn send_message(&self, payload: Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let state = self.state.read().await;
        if let Some(tx) = &state.connection {
            tx.send(Message::Text(payload.to_string()))?;
            Ok(())
        } else {
            Err("Not connected".into())
        }
    }

    pub async fn get_current_scene(&self) -> Result<OBSScene, Box<dyn std::error::Error + Send + Sync>> {
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "GetCurrentProgramScene",
                "requestId": "get_current_scene"
            }
        });

        self.send_message(payload).await?;

        // Here you would typically wait for and parse the response
        // For now, we'll just return a dummy scene
        Ok(OBSScene {
            name: "Dummy Scene".to_string(),
            items: vec![],
        })
    }

    pub async fn set_current_scene(&self, scene_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "SetCurrentProgramScene",
                "requestId": "set_current_scene",
                "requestData": {
                    "sceneName": scene_name
                }
            }
        });

        self.send_message(payload).await?;

        // Here you would typically wait for a response to confirm the scene change
        // For simplicity, we're just assuming it succeeded

        Ok(())
    }

    pub async fn get_scene_list(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "GetSceneList",
                "requestId": "get_scene_list"
            }
        });

        self.send_message(payload).await?;

        // In a real implementation, you would wait for and parse the response
        // For now, we'll return a dummy list
        Ok(vec!["Scene 1".to_string(), "Scene 2".to_string()])
    }

    pub async fn get_scene_items(&self, scene_name: &str) -> Result<Vec<OBSSceneItem>, Box<dyn std::error::Error + Send + Sync>> {
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "GetSceneItemList",
                "requestId": "get_scene_items",
                "requestData": {
                    "sceneName": scene_name
                }
            }
        });

        self.send_message(payload).await?;

        // In a real implementation, you would wait for and parse the response
        // For now, we'll return dummy items
        Ok(vec![
            OBSSceneItem {
                name: "Item 1".to_string(),
                source_type: "image_source".to_string(),
                visible: true,
            },
            OBSSceneItem {
                name: "Item 2".to_string(),
                source_type: "browser_source".to_string(),
                visible: true,
            },
        ])
    }

    pub async fn set_scene_item_enabled(&self, scene_name: &str, item_name: &str, enabled: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "SetSceneItemEnabled",
                "requestId": "set_scene_item_enabled",
                "requestData": {
                    "sceneName": scene_name,
                    "sceneItemName": item_name,
                    "sceneItemEnabled": enabled
                }
            }
        });

        self.send_message(payload).await?;

        // In a real implementation, you would wait for and check the response
        Ok(())
    }

    pub async fn refresh_browser_source(&self, source_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "PressInputPropertiesButton",
                "requestId": "refresh_browser_source",
                "requestData": {
                    "inputName": source_name,
                    "propertyName": "refreshNoCache"
                }
            }
        });

        self.send_message(payload).await?;

        // In a real implementation, you would wait for and check the response
        Ok(())
    }
}



impl OBSManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_instance(&self, name: String, instance: OBSInstance) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = OBSWebSocketClient::new(instance);
        match client.connect().await {
            Ok(_) => {
                self.clients.write().await.insert(name.clone(), client);
                info!("Successfully connected to OBS instance: {}", name);
                Ok(())
            }
            Err(e) => {
                error!("Failed to connect to OBS instance {}: {}. This instance will be unavailable.", name, e);
                Ok(()) // We return Ok to allow the program to continue with other instances
            }
        }
    }

    pub async fn remove_instance(&self, name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.clients.write().await.remove(name);
        Ok(())
    }

    pub async fn get_client(&self, name: &str) -> Option<OBSWebSocketClient> {
        self.clients.read().await.get(name).cloned()
    }

    pub async fn get_scene_list(&self, instance_name: &str) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client(instance_name).await.ok_or("OBS instance not found")?;
        client.get_scene_list().await
    }

    pub async fn get_scene_items(&self, instance_name: &str, scene_name: &str) -> Result<Vec<OBSSceneItem>, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client(instance_name).await.ok_or("OBS instance not found")?;
        client.get_scene_items(scene_name).await
    }

    pub async fn set_current_scene(&self, instance_name: &str, scene_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self.get_client(instance_name).await {
            Some(client) => client.set_current_scene(scene_name).await,
            None => {
                warn!("OBS instance {} not found. Cannot set current scene.", instance_name);
                Ok(()) // We return Ok to allow the program to continue with other operations
            }
        }
    }

    pub async fn set_scene_item_enabled(&self, instance_name: &str, scene_name: &str, source_name: &str, enabled: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self.get_client(instance_name).await {
            Some(client) => client.set_scene_item_enabled(scene_name, source_name, enabled).await,
            None => {
                warn!("OBS instance {} not found. Cannot set scene item enabled state.", instance_name);
                Ok(()) // We return Ok to allow the program to continue with other operations
            }
        }
    }

    pub async fn refresh_browser_source(&self, instance_name: &str, source_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match self.get_client(instance_name).await {
            Some(client) => client.refresh_browser_source(source_name).await,
            None => {
                warn!("OBS instance {} not found. Cannot refresh browser source.", instance_name);
                Ok(()) // We return Ok to allow the program to continue with other operations
            }
        }
    }

    pub async fn get_instances(&self) -> Vec<OBSInstanceState> {
        let clients = self.clients.read().await;
        let mut instances = Vec::new();

        for (name, client) in clients.iter() {
            match self.get_instance_state(name, client).await {
                Ok(state) => instances.push(state),
                Err(e) => warn!("Failed to get state for OBS instance {}: {}. This instance will be skipped.", name, e),
            }
        }

        instances
    }

    async fn get_instance_state(&self, name: &str, client: &OBSWebSocketClient) -> Result<OBSInstanceState, Box<dyn std::error::Error + Send + Sync>> {
        let scenes = client.get_scene_list().await?;
        let current_scene = client.get_current_scene().await?;
        let mut sources = HashMap::new();

        for scene in &scenes {
            match client.get_scene_items(scene).await {
                Ok(items) => {
                    sources.insert(scene.clone(), items);
                }
                Err(e) => {
                    warn!("Failed to get scene items for scene {} in instance {}: {}. This scene will be skipped.", scene, name, e);
                }
            }
        }

        Ok(OBSInstanceState {
            name: name.to_string(),
            scenes,
            current_scene: current_scene.name,
            sources,
        })
    }
}