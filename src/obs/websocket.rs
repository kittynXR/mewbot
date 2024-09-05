use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::{mpsc, RwLock, oneshot, Mutex};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn, debug};
use serde::{Deserialize};
use tokio_tungstenite::{connect_async_with_config, tungstenite::protocol::Message, tungstenite::client::IntoClientRequest, Connector, WebSocketStream, MaybeTlsStream};
use tokio_tungstenite::tungstenite::http::Uri;
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};
use sha256::digest;
use base64::{engine::general_purpose, Engine as _};
use futures_util::stream::SplitSink;
use native_tls::TlsConnector;
use tokio::net::TcpStream;
use crate::obs::models::{OBSInstance, OBSScene, OBSSceneItem};
use crate::obs::{OBSClientState, OBSInstanceState, OBSWebSocketClient};

pub const TIMEOUT_DURATION: Duration = Duration::from_millis(1000);
pub const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(10);
const MAX_RECONNECT_ATTEMPTS: u32 = 5;

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, Deserialize)]
pub struct OBSResponse {
    op: u8,
    d: Value,
}

#[derive(Debug, Deserialize)]
pub struct HelloMessage {
    #[serde(rename = "obsWebSocketVersion")]
    #[allow(dead_code)]
    obs_web_socket_version: String,

    #[serde(rename = "rpcVersion")]
    #[allow(dead_code)]
    rpc_version: i32,

    authentication: Option<AuthenticationInfo>,
}

#[derive(Debug, Deserialize)]
pub struct AuthenticationInfo {
    challenge: String,
    salt: String,
}

impl OBSWebSocketClient {
    pub fn new(instance: OBSInstance) -> Self {
        Self {
            instance,
            state: Arc::new(RwLock::new(OBSClientState {
                connection: None,
                connection_state: ConnectionState::Disconnected,
            })),
            response_channels: Arc::new(RwLock::new(HashMap::new())),
            connection_task: Arc::new(Mutex::new(None)),
            should_reconnect: Arc::new(AtomicBool::new(true)),
        }
    }

    pub async fn connect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut connection_task = self.connection_task.lock().await;
        if connection_task.is_some() {
            return Ok(()); // Connection task is already running
        }

        self.should_reconnect.store(true, Ordering::SeqCst);
        let client = self.clone();
        *connection_task = Some(tokio::spawn(async move {
            client.connection_manager().await;
        }));

        Ok(())
    }

    async fn connection_manager(&self) {
        let mut retry_delay = Duration::from_millis(100);
        let mut attempt = 0;

        while self.should_reconnect.load(Ordering::SeqCst) {
            {
                let state = self.state.read().await;
                if state.connection_state == ConnectionState::Connected {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }
            }

            {
                let mut state = self.state.write().await;
                state.connection_state = ConnectionState::Connecting;
            }

            info!("Attempting to connect to OBS instance: {}", self.instance.address);
            match timeout(Duration::from_secs(30), self.attempt_connect()).await {
                Ok(result) => {
                    match result {
                        Ok(_) => {
                            info!("Successfully connected to OBS instance: {}", self.instance.address);
                            self.state.write().await.connection_state = ConnectionState::Connected;
                            attempt = 0;
                            retry_delay = Duration::from_millis(100);
                        }
                        Err(e) => {
                            error!("Failed to connect to OBS instance {}: {}. Retrying in {:?}...", self.instance.address, e, retry_delay);
                            self.state.write().await.connection_state = ConnectionState::Disconnected;
                        }
                    }
                }
                Err(_) => {
                    error!("Connection attempt to OBS instance {} timed out. Retrying in {:?}...", self.instance.address, retry_delay);
                    self.state.write().await.connection_state = ConnectionState::Disconnected;
                }
            }

            if self.state.read().await.connection_state != ConnectionState::Connected {
                attempt += 1;
                if attempt >= MAX_RECONNECT_ATTEMPTS {
                    error!("Max reconnection attempts reached for OBS instance: {}", self.instance.address);
                    break;
                }

                tokio::time::sleep(retry_delay).await;
                retry_delay = std::cmp::min(retry_delay * 2, MAX_RECONNECT_DELAY);
            }
        }
    }

    pub async fn disconnect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.should_reconnect.store(false, Ordering::SeqCst);
        let mut connection_task = self.connection_task.lock().await;
        if let Some(task) = connection_task.take() {
            task.abort();
        }

        let mut state = self.state.write().await;
        if let Some(tx) = state.connection.take() {
            drop(tx); // This should close the channel and stop the write loop
        }
        state.connection_state = ConnectionState::Disconnected;
        Ok(())
    }

    pub fn attempt_connect(&self) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + '_>> {
        Box::pin(async move {
            info!("Starting connection attempt to OBS instance: {}", self.instance.address);
            let scheme = if self.instance.use_ssl { "wss" } else { "ws" };
            let url = format!("{}://{}:{}", scheme, self.instance.address, self.instance.port);
            let uri: Uri = url.parse()?;
            let request = uri.into_client_request()?;

            let _connector = if self.instance.use_ssl {
                Some(Connector::NativeTls(TlsConnector::builder().build()?))
            } else {
                None
            };

            let (ws_stream, _) = connect_async_with_config(
                request,
                None,
                self.instance.use_ssl
            ).await?;

            let (write, read) = ws_stream.split();

            let (tx, rx) = mpsc::unbounded_channel();

            // Spawn the write loop
            self.spawn_write_loop(write, rx, self.instance.name.clone());

            let mut state = self.state.write().await;
            state.connection = Some(tx.clone());
            drop(state);  // Release the write lock

            let client_clone = self.clone();
            tokio::spawn(async move {
                client_clone.handle_incoming_messages(read).await;
            });

            info!("WebSocket connection established. Waiting for Hello message...");
            let hello_message = match tokio::time::timeout(Duration::from_secs(10), self.wait_for_hello()).await {
                Ok(result) => match result {
                    Ok(msg) => msg,
                    Err(e) => {
                        error!("Error while waiting for Hello message: {:?}", e);
                        return Err(e);
                    }
                },
                Err(_) => {
                    error!("Timeout while waiting for Hello message");
                    return Err("Timeout while waiting for Hello message".into());
                }
            };
            info!("Received Hello message: {:?}", hello_message);

            // Handle authentication if required
            if let Some(auth_info) = hello_message.authentication {
                if self.instance.auth_required {
                    info!("Authentication required. Attempting to authenticate...");
                    self.authenticate(&auth_info).await?;
                    info!("Authentication successful.");
                } else {
                    warn!("Server requires authentication, but auth_required is set to false for this instance.");
                }
            } else if self.instance.auth_required {
                warn!("Instance is configured to require authentication, but server does not require it.");
            }

            info!("Sending Identify message...");
            let identify_payload = json!({
                "op": 1,
                "d": {
                    "rpcVersion": 1,
                    "authentication": 0,
                    "eventSubscriptions": 33
                }
            });
            debug!("Identify payload: {:?}", identify_payload);

            match self.send_message(identify_payload.clone()).await {
                Ok(_) => info!("Identify message sent successfully."),
                Err(e) => {
                    error!("Failed to send Identify message: {}", e);
                    return Err(e.into());
                }
            }

            info!("Waiting for Identified message...");
            let (tx, rx) = oneshot::channel();
            self.response_channels.write().await.insert("identify".to_string(), tx);

            let timeout_duration = Duration::from_secs(5);
            match timeout(timeout_duration, rx).await {
                Ok(result) => {
                    match result {
                        Ok(response) => {
                            info!("Received Identified message: {:?}", response);
                            info!("Connection process complete.");
                            Ok(())
                        },
                        Err(e) => {
                            error!("Error receiving Identified message: {}", e);
                            Err("Failed to receive Identified message".into())
                        }
                    }
                },
                Err(_) => {
                    error!("Timeout waiting for Identified message");
                    Err("Timeout waiting for Identified message".into())
                }
            }
        })
    }

    async fn handle_incoming_messages(&self, mut read: impl StreamExt<Item = Result<tungstenite::Message, tokio_tungstenite::tungstenite::Error>> + Unpin + Send + 'static) {
        debug!("Started handling incoming messages");
        while let Some(message) = read.next().await {
            match message {
                Ok(tungstenite::Message::Text(text)) => {
                    debug!("Received message: {}", text);
                    match serde_json::from_str::<OBSResponse>(&text) {
                        Ok(response) => {
                            self.handle_response(response).await;
                        }
                        Err(e) => {
                            error!("Failed to parse OBS response: {}", e);
                        }
                    }
                }
                Ok(tungstenite::Message::Close(frame)) => {
                    info!("WebSocket connection closed: {:?}", frame);
                    break;
                }
                Err(e) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                _ => {
                    debug!("Received non-text message: {:?}", message);
                }
            }
        }
        // Attempt to reconnect
        info!("Connection closed, attempting to reconnect...");
        if let Err(e) = self.connect().await {
            error!("Failed to reconnect: {:?}", e);
        }
    }

    async fn handle_response(&self, response: OBSResponse) {
        debug!("Received response: {:?}", response);
        match response.op {
            0 => {
                debug!("Received Hello message");
                if let Some(tx) = self.response_channels.write().await.remove("hello") {
                    let _ = tx.send(response.d);
                }
            }
            2 => {
                info!("Received Identified message: {:?}", response);
                if let Some(tx) = self.response_channels.write().await.remove("identify") {
                    let _ = tx.send(response.d);
                } else {
                    warn!("Received Identified message but no receiver was waiting for it");
                }
            }
            7 => {
                debug!("Received request response: {:?}", response.d);
                if let Some(request_id) = response.d["requestId"].as_str() {
                    if let Some(tx) = self.response_channels.write().await.remove(request_id) {
                        let _ = tx.send(response.d);
                    }
                }
            }
            _ => {
                warn!("Received unknown op code: {}", response.op);
            }
        }
    }

    fn spawn_write_loop(
        &self,
        mut write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        mut rx: mpsc::UnboundedReceiver<Message>,
        instance_name: String,
    ) {
        tokio::spawn(async move {
            debug!("[{}] Write loop started", instance_name);
            while let Some(message) = rx.recv().await {
                debug!("[{}] Write loop received message: {:?}", instance_name, message);
                match write.send(message).await {
                    Ok(_) => debug!("[{}] Message sent successfully in write loop", instance_name),
                    Err(e) => {
                        error!("[{}] Error sending message in write loop: {:?}", instance_name, e);
                        // We can't call self.reconnect() here, so we'll just break the loop
                        break;
                    }
                }
            }
            debug!("[{}] Write loop ended", instance_name);
        });
    }

    async fn send_message(&self, payload: serde_json::Value) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let state = self.state.read().await;
        if let Some(tx) = &state.connection {
            let message = tungstenite::Message::Text(payload.to_string());
            debug!("[{}] Attempting to send message: {:?}", self.instance.name, message);
            match tx.send(message) {
                Ok(_) => {
                    debug!("[{}] Message sent successfully to channel", self.instance.name);
                    Ok(())
                },
                Err(e) => {
                    error!("[{}] Failed to send message to channel: {:?}", self.instance.name, e);
                    // We'll handle reconnection in the caller
                    Err(Box::new(e))
                }
            }
        } else {
            error!("[{}] Attempted to send message but not connected", self.instance.name);
            // We'll handle reconnection in the caller
            Err("Not connected".into())
        }
    }

    async fn wait_for_hello(&self) -> Result<HelloMessage, Box<dyn std::error::Error + Send + Sync>> {
        let (tx, rx) = oneshot::channel();
        self.response_channels.write().await.insert("hello".to_string(), tx);

        let response = timeout(TIMEOUT_DURATION, rx).await??;
        let hello_message: HelloMessage = serde_json::from_value(response)?;
        Ok(hello_message)
    }

    async fn authenticate(&self, auth_info: &AuthenticationInfo) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(password) = &self.instance.password {
            let auth_response = self.generate_auth_response(password, &auth_info.challenge, &auth_info.salt);

            let auth_payload = json!({
                "op": 1,
                "d": {
                    "rpcVersion": 1,
                    "authentication": auth_response,
                }
            });

            let (tx, rx) = oneshot::channel();
            self.response_channels.write().await.insert("auth".to_string(), tx);

            self.send_message(auth_payload).await?;

            // Wait for authentication response
            timeout(TIMEOUT_DURATION, rx).await??;
        } else {
            return Err("Authentication required but no password provided".into());
        }

        Ok(())
    }

    fn generate_auth_response(&self, password: &str, challenge: &str, salt: &str) -> String {
        let secret_string = format!("{}{}", password, salt);
        let secret_hash = digest(secret_string);
        let auth_response_string = format!("{}{}", secret_hash, challenge);
        let auth_response_hash = digest(auth_response_string);
        general_purpose::STANDARD_NO_PAD.encode(auth_response_hash)
    }

    pub async fn get_instance_state(&self) -> Result<OBSInstanceState, Box<dyn std::error::Error + Send + Sync>> {
        let scenes = self.get_scene_list().await?;
        let current_scene = self.get_current_scene().await?;
        let mut sources = HashMap::new();

        for scene in &scenes {
            let items = self.get_scene_items(&scene).await?;
            sources.insert(scene.clone(), items);
        }

        Ok(OBSInstanceState {
            name: self.instance.name.clone(),
            scenes,
            current_scene: current_scene.name,
            sources,
        })
    }

    pub async fn get_current_scene(&self) -> Result<OBSScene, Box<dyn std::error::Error + Send + Sync>> {
        let request_id = "get_current_scene";
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "GetCurrentProgramScene",
                "requestId": request_id
            }
        });

        let response = self.send_request(payload, request_id).await?;

        let scene_name = response["currentProgramSceneName"].as_str()
            .ok_or("Invalid response: missing scene name")?;

        Ok(OBSScene {
            name: scene_name.to_string(),
            items: vec![], // You might want to fetch scene items in a separate request
        })
    }

    pub async fn set_current_scene(&self, scene_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let request_id = "set_current_scene";
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "SetCurrentProgramScene",
                "requestId": request_id,
                "requestData": {
                    "sceneName": scene_name
                }
            }
        });

        self.send_request(payload, request_id).await?;

        Ok(())
    }

    async fn get_scene_list(&self) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let request_id = "get_scene_list";
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "GetSceneList",
                "requestId": request_id
            }
        });

        let response = self.send_request(payload, request_id).await?;

        let scenes = response["responseData"]["scenes"].as_array()
            .ok_or("Invalid response: missing scenes array")?;

        Ok(scenes.iter()
            .filter_map(|scene| scene["sceneName"].as_str().map(String::from))
            .collect())
    }

    pub async fn get_scene_items(&self, scene_name: &str) -> Result<Vec<OBSSceneItem>, Box<dyn std::error::Error + Send + Sync>> {
        let request_id = "get_scene_items";
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "GetSceneItemList",
                "requestId": request_id,
                "requestData": {
                    "sceneName": scene_name
                }
            }
        });

        let response = self.send_request(payload, request_id).await?;

        let items = response["sceneItems"].as_array()
            .ok_or("Invalid response: missing sceneItems array")?;

        Ok(items.iter()
            .filter_map(|item| {
                let name = item["sourceName"].as_str()?;
                let source_type = item["inputKind"].as_str()?;
                let visible = item["sceneItemEnabled"].as_bool().unwrap_or(false);
                Some(OBSSceneItem {
                    name: name.to_string(),
                    source_type: source_type.to_string(),
                    visible,
                })
            })
            .collect())
    }

    pub async fn set_scene_item_enabled(&self, scene_name: &str, item_name: &str, enabled: bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let request_id = "set_scene_item_enabled";
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "SetSceneItemEnabled",
                "requestId": request_id,
                "requestData": {
                    "sceneName": scene_name,
                    "sceneItemId": item_name,
                    "sceneItemEnabled": enabled
                }
            }
        });

        self.send_request(payload, request_id).await?;

        Ok(())
    }

    pub async fn refresh_browser_source(&self, source_name: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let request_id = "refresh_browser_source";
        let payload = json!({
            "op": 6,
            "d": {
                "requestType": "PressInputPropertiesButton",
                "requestId": request_id,
                "requestData": {
                    "inputName": source_name,
                    "propertyName": "refreshNoCache"
                }
            }
        });

        self.send_request(payload, request_id).await?;

        Ok(())
    }

    async fn send_request(&self, payload: Value, request_id: &str) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let (tx, rx) = oneshot::channel();
        self.response_channels.write().await.insert(request_id.to_string(), tx);

        self.send_message(payload).await?;

        // Wait for the response
        let response = timeout(TIMEOUT_DURATION, rx).await??;

        Ok(response)
    }
}

impl Clone for OBSWebSocketClient {
    fn clone(&self) -> Self {
        Self {
            instance: self.instance.clone(),
            state: Arc::clone(&self.state),
            response_channels: Arc::clone(&self.response_channels),
            connection_task: Arc::clone(&self.connection_task),
            should_reconnect: Arc::clone(&self.should_reconnect),
        }
    }
}