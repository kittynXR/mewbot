use std::sync::Arc;
use tokio::sync::RwLock;
use warp::ws::{Message, WebSocket};
use futures::{FutureExt, StreamExt};
use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use crate::{log_debug, log_error, log_info, log_warn};
use crate::LogLevel;
use crate::storage::StorageClient;
use crate::logging::Logger;
use super::storage_ext::StorageClientExt;
use super::websocket_server::DashboardState;

#[derive(Serialize, Deserialize)]
pub struct WebSocketMessage {
    pub message_type: String,
    pub data: serde_json::Value,
}

pub async fn handle_websocket(
    ws: WebSocket,
    dashboard_state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>
) {
    log_info!(logger, "New WebSocket connection established");
    let (mut ws_tx, mut ws_rx) = ws.split();

    while let Some(result) = ws_rx.next().await {
        match result {
            Ok(msg) => {
                if let Ok(text) = msg.to_str() {
                    log_debug!(logger, "Received WebSocket message: {}", text);
                    if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(text) {
                        let response = handle_ws_message(&ws_message, &dashboard_state, &storage, &logger).await;
                        if let Ok(response_str) = serde_json::to_string(&response) {
                            log_debug!(logger, "Sending WebSocket response: {}", response_str);
                            if ws_tx.send(Message::text(response_str)).await.is_err() {
                                log_error!(logger, "Failed to send WebSocket message");
                                break;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log_error!(logger, "WebSocket error: {:?}", e);
                break;
            }
        }
    }
    log_info!(logger, "WebSocket connection closed");
}

async fn handle_ws_message(
    message: &WebSocketMessage,
    dashboard_state: &Arc<RwLock<DashboardState>>,
    storage: &Arc<RwLock<StorageClient>>,
    logger: &Arc<Logger>
) -> WebSocketMessage {
    match message.message_type.as_str() {
        "get_bot_status" => {
            log_info!(logger, "Fetching bot status");
            let dashboard_state = dashboard_state.read().await;
            let bot_status = dashboard_state.bot_status.read().await;
            WebSocketMessage {
                message_type: "bot_status".to_string(),
                data: serde_json::json!({
                    "status": if bot_status.is_online() { "online" } else { "offline" },
                    "uptime": bot_status.uptime_string(),
                    "active_modules": ["twitch", "discord", "vrchat"] // You may want to implement this properly
                }),
            }
        }
        "shareWorld" => {
            log_info!(logger, "Sharing VRChat world");
            let dashboard_state = dashboard_state.read().await;
            if let Some(world) = &dashboard_state.vrchat_world {
                let world_info = format!("Current VRChat World: {} by {}", world.name, world.author_name);
                if let Ok(twitch_channel) = dashboard_state.get_twitch_channel().await {
                    let twitch_client = dashboard_state.get_twitch_client();
                    if let Err(e) = twitch_client.send_message(&twitch_channel, &world_info).await {
                        log_error!(logger, "Error sending world info to Twitch chat: {:?}", e);
                    }
                }
            }
            WebSocketMessage {
                message_type: "worldShared".to_string(),
                data: serde_json::json!({"status": "success"}),
            }
        }
        "sendChat" => {
            if let Some(chat_msg) = message.data.get("message").and_then(|m| m.as_str()) {
                log_info!(logger, "Sending chat message: {}", chat_msg);
                let dashboard_state = dashboard_state.read().await;
                let destinations = &message.data["destination"];
                if destinations["oscTextbox"].as_bool().unwrap_or(false) {
                    if let Some(vrchat_osc) = dashboard_state.get_vrchat_osc() {
                        if let Err(e) = vrchat_osc.send_chatbox_message(chat_msg, true, false) {
                            log_error!(logger, "Error sending message to VRChat OSC: {:?}", e);
                        }
                    }
                }
                if destinations["twitchChat"].as_bool().unwrap_or(false) {
                    if let Ok(twitch_channel) = dashboard_state.get_twitch_channel().await {
                        let twitch_client = dashboard_state.get_twitch_client();
                        if let Err(e) = twitch_client.send_message(&twitch_channel, chat_msg).await {
                            log_error!(logger, "Error sending message to Twitch chat: {:?}", e);
                        }
                    }
                }
                WebSocketMessage {
                    message_type: "chatSent".to_string(),
                    data: serde_json::json!({"status": "success"}),
                }
            } else {
                WebSocketMessage {
                    message_type: "error".to_string(),
                    data: serde_json::json!({"error": "Invalid chat message"}),
                }
            }
        }
        _ => {
            log_warn!(logger, "Unknown WebSocket message type: {}", message.message_type);
            WebSocketMessage {
                message_type: "error".to_string(),
                data: serde_json::json!({"error": "Unknown message type"}),
            }
        },
    }
}