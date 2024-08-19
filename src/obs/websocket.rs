// src/obs/websocket.rs

use std::collections::HashMap;
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::protocol::Message,
    tungstenite::client::IntoClientRequest,
    tungstenite::handshake::client::generate_key
};
use tokio_tungstenite::tungstenite::http::Uri;
use serde_json::{json, Value};
use crate::obs::models::{ObsInstance, ObsScene};

#[derive(Clone)]
pub struct ObsWebSocketClient {
    instance: ObsInstance,
    state: Arc<RwLock<ObsClientState>>,
}

struct ObsClientState {
    connection: Option<mpsc::UnboundedSender<Message>>,
}

impl ObsWebSocketClient {
    pub fn new(instance: ObsInstance) -> Self {
        Self {
            instance,
            state: Arc::new(RwLock::new(ObsClientState {
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

    pub async fn get_current_scene(&self) -> Result<ObsScene, Box<dyn std::error::Error + Send + Sync>> {
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
        Ok(ObsScene {
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
}
pub struct ObsManager {
    clients: Arc<RwLock<HashMap<String, ObsWebSocketClient>>>,
}

impl ObsManager {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_instance(&self, name: String, instance: ObsInstance) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = ObsWebSocketClient::new(instance);
        client.connect().await?;
        self.clients.write().await.insert(name, client);
        Ok(())
    }

    pub async fn remove_instance(&self, name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.clients.write().await.remove(name);
        Ok(())
    }

    pub async fn get_client(&self, name: &str) -> Option<ObsWebSocketClient> {
        self.clients.read().await.get(name).cloned()
    }

    pub async fn get_current_scene(&self, instance_name: &str) -> Result<ObsScene, Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client(instance_name).await.ok_or("OBS instance not found")?;
        client.get_current_scene().await
    }

    pub async fn set_current_scene(&self, instance_name: &str, scene_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client = self.get_client(instance_name).await.ok_or("OBS instance not found")?;
        client.set_current_scene(scene_name).await
    }

    // Add more methods that work across all instances or target specific instances
}