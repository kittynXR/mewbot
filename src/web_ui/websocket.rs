use std::sync::Arc;
use tokio::sync::RwLock;
use warp::ws::{Message, WebSocket};
use futures::StreamExt;
use futures_util::SinkExt;
use serde::{Deserialize, Serialize};
use serde_json::from_str;
use crate::{log_error, log_info};
use crate::LogLevel;
use crate::storage::StorageClient;
use crate::logging::Logger;
use super::websocket_server::DashboardState;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WebSocketMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub message: Option<String>,
    pub destination: Option<ChatDestination>,
    pub world: Option<serde_json::Value>,
    pub additional_streams: Option<Vec<String>>,
    pub user_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatDestination {
    pub oscTextbox: bool,
    pub twitchChat: bool,
    pub twitchBot: bool,
    pub twitchBroadcaster: bool,
}

pub async fn handle_websocket(
    ws: WebSocket,
    dashboard_state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>
) {
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Create a new receiver for this connection
    let mut rx = dashboard_state.read().await.tx.subscribe();

    // Send initial status
    let initial_status = {
        let dashboard_state = dashboard_state.read().await;
        let bot_status = dashboard_state.bot_status.read().await;
        WebSocketMessage {
            message_type: "bot_status".to_string(),
            message: Some(if bot_status.is_online() { "online".to_string() } else { "offline".to_string() }),
            destination: None,
            world: Some(serde_json::json!({
                "uptime": bot_status.uptime_string(),
                "active_modules": ["twitch", "discord", "vrchat"] // You may want to implement this properly
            })),
            additional_streams: None,
            user_id: None,
        }
    };

    if let Ok(initial_status_str) = serde_json::to_string(&initial_status) {
        log_info!(logger, "Sending initial status: {}", initial_status_str);
        if let Err(e) = ws_tx.send(Message::text(initial_status_str)).await {
            log_error!(logger, "Failed to send initial WebSocket message: {:?}", e);
            return;
        }
    }

    loop {
        tokio::select! {
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(msg)) => {
                        if let Ok(text) = msg.to_str() {
                            log_info!(logger, "Received WebSocket message: {}", text);
                            match from_str::<WebSocketMessage>(text) {
                                Ok(parsed_message) => {
                                    if let Err(e) = handle_ws_message(&parsed_message, &dashboard_state, &storage, &logger, &mut ws_tx).await {
                                        log_error!(logger, "Error handling WebSocket message: {:?}", e);
                                    }
                                }
                                Err(e) => {
                                    log_error!(logger, "Failed to parse WebSocket message: {:?}", e);
                                    log_error!(logger, "Raw message: {}", text);
                                }
                            }
                        }
                    }
                    Some(Err(e)) => {
                        log_error!(logger, "WebSocket error: {:?}", e);
                        break;
                    }
                    None => break,
                }
            }
            update = rx.recv() => {
                match update {
                    Ok(msg) => {
                        if let Ok(msg_str) = serde_json::to_string(&msg) {
                            if let Err(e) = ws_tx.send(Message::text(msg_str)).await {
                                log_error!(logger, "Failed to send WebSocket message: {:?}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        log_error!(logger, "Failed to receive broadcast message: {:?}", e);
                        break;
                    }
                }
            }
        }
    }
    log_info!(logger, "WebSocket connection closed");
}

async fn handle_ws_message(
    message: &WebSocketMessage,
    dashboard_state: &Arc<RwLock<DashboardState>>,
    storage: &Arc<RwLock<StorageClient>>,
    logger: &Arc<Logger>,
    ws_tx: &mut futures::stream::SplitSink<WebSocket, Message>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let dashboard_state = dashboard_state.read().await;

    match message.message_type.as_str() {
        "sendChat" => {
            if let Some(chat_msg) = &message.message {
                log_info!(logger, "Sending chat message: {}", chat_msg);
                if let Some(destinations) = &message.destination {
                    if destinations.oscTextbox {
                        if let Some(vrchat_osc) = dashboard_state.get_vrchat_osc() {
                            if let Err(e) = vrchat_osc.send_chatbox_message(chat_msg, true, false) {
                                log_error!(logger, "Error sending message to VRChat OSC: {:?}", e);
                            }
                        }
                    }
                    if destinations.twitchChat {
                        if let Ok(twitch_channel) = dashboard_state.get_twitch_channel().await {
                            if destinations.twitchBot {
                                if let Some(twitch_client) = dashboard_state.get_twitch_bot_client().await {
                                    if let Err(e) = twitch_client.send_message(&twitch_channel, chat_msg).await {
                                        log_error!(logger, "Error sending message to Twitch chat as bot: {:?}", e);
                                    }
                                }
                            }
                            if destinations.twitchBroadcaster {
                                if let Some(twitch_client) = dashboard_state.get_twitch_broadcaster_client().await {
                                    if let Err(e) = twitch_client.send_message(&twitch_channel, chat_msg).await {
                                        log_error!(logger, "Error sending message to Twitch chat as broadcaster: {:?}", e);
                                    }
                                }
                            }
                        }
                    }
                }

                // Handle additional streams
                if let Some(additional_streams) = &message.additional_streams {
                    for stream in additional_streams {
                        if let Some(twitch_client) = dashboard_state.get_twitch_bot_client().await {
                            if let Err(e) = twitch_client.send_message(stream, chat_msg).await {
                                log_error!(logger, "Error sending message to additional stream {}: {:?}", stream, e);
                            }
                        }
                    }
                }

                // Send a response back to the client
                let response = WebSocketMessage {
                    message_type: "chatSent".to_string(),
                    message: Some("success".to_string()),
                    destination: None,
                    world: None,
                    additional_streams: None,
                    user_id: None,
                };
                ws_tx.send(Message::text(serde_json::to_string(&response)?)).await?;
            }
        }
        "shareWorld" => {
            log_info!(logger, "Sharing VRChat world");
            if let Some(world) = &dashboard_state.vrchat_world {
                let world_info = format!("Current VRChat World: {} by {}", world.name, world.author_name);
                if let Ok(twitch_channel) = dashboard_state.get_twitch_channel().await {
                    if let Some(twitch_client) = dashboard_state.get_twitch_bot_client().await {
                        if let Err(e) = twitch_client.send_message(&twitch_channel, &world_info).await {
                            log_error!(logger, "Error sending world info to Twitch chat: {:?}", e);
                        }
                    }
                }
            }
            // Send a response back to the client
            let response = WebSocketMessage {
                message_type: "worldShared".to_string(),
                message: Some("success".to_string()),
                destination: None,
                world: None,
                additional_streams: None,
                user_id: None,
            };
            ws_tx.send(Message::text(serde_json::to_string(&response)?)).await?;
        }
        "twitch_message" => {
            if let (Some(chat_msg), Some(user_id)) = (&message.message, &message.user_id) {
                log_info!(logger, "Received Twitch message from {}: {}", user_id, chat_msg);

                // Add the message to recent messages
                let storage = storage.read().await;
                if let Err(e) = storage.add_message(user_id, chat_msg) {
                    log_error!(logger, "Error adding message to storage: {:?}", e);
                }

                // Broadcast the message to all connected clients
                let broadcast_msg = WebSocketMessage {
                    message_type: "twitch_message".to_string(),
                    message: Some(chat_msg.clone()),
                    user_id: Some(user_id.clone()),
                    destination: None,
                    world: None,
                    additional_streams: None,
                };
                if let Err(e) = dashboard_state.broadcast_message(broadcast_msg).await {
                    log_error!(logger, "Error broadcasting Twitch message: {:?}", e);
                }
            } else {
                log_error!(logger, "Received incomplete Twitch message");
            }
        }
        _ => {
            log_error!(logger, "Unknown message type: {}", message.message_type);
        }
    }
    Ok(())
}