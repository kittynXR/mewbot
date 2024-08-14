use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use warp::ws::{Message, WebSocket};
use futures::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::Config;
use crate::vrchat::models::World;
use crate::twitch::irc::{TwitchIRCManager, TwitchBotClient, TwitchBroadcasterClient};
use crate::storage::StorageClient;
use crate::bot_status::BotStatus;
use crate::discord::DiscordClient;
use crate::{log_error, log_info};
use crate::LogLevel;
use crate::logging::Logger;
use crate::osc::VRChatOSC;
use crate::web_ui::storage_ext::StorageClientExt;

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

pub struct DashboardState {
    pub(crate) bot_status: Arc<RwLock<BotStatus>>,
    pub(crate) vrchat_world: Option<World>,
    pub(crate) twitch_status: bool,
    pub(crate) discord_status: bool,
    pub(crate) vrchat_status: bool,
    pub(crate) obs_status: bool,
    recent_messages: Vec<String>,
    config: Arc<RwLock<Config>>,
    twitch_irc_manager: Option<Arc<TwitchIRCManager>>,
    vrchat_osc: Option<Arc<VRChatOSC>>,
    pub(crate) tx: broadcast::Sender<WebSocketMessage>,
    pub(crate) rx: broadcast::Receiver<WebSocketMessage>,
}

impl DashboardState {
    pub fn new(
        bot_status: Arc<RwLock<BotStatus>>,
        config: Arc<RwLock<Config>>,
        twitch_irc_manager: Option<Arc<TwitchIRCManager>>,
        vrchat_osc: Option<Arc<VRChatOSC>>,
    ) -> Self {
        let (tx, rx) = broadcast::channel::<WebSocketMessage>(100);
        Self {
            bot_status,
            vrchat_world: None,
            twitch_status: twitch_irc_manager.is_some(),
            discord_status: false,
            vrchat_status: vrchat_osc.is_some(),
            obs_status: false,
            recent_messages: Vec::new(),
            config,
            twitch_irc_manager,
            vrchat_osc,
            tx,
            rx,
        }
    }

    pub async fn broadcast_message(&self, message: WebSocketMessage) -> Result<usize, broadcast::error::SendError<WebSocketMessage>> {
        self.tx.send(message)
    }


    pub async fn get_twitch_channel(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.config.read().await.twitch_channel_to_join
            .clone()
            .ok_or_else(|| "Twitch channel not set".into())
    }

    pub async fn get_twitch_bot_client(&self) -> Option<TwitchBotClient> {
        let config = self.config.read().await;
        self.twitch_irc_manager.as_ref().map(|manager| {
            TwitchBotClient::new(
                config.twitch_bot_username.clone().unwrap(),
                manager.clone(),
            )
        })
    }

    pub async fn get_twitch_broadcaster_client(&self) -> Option<TwitchBroadcasterClient> {
        let config = self.config.read().await;
        self.twitch_irc_manager.as_ref().map(|manager| {
            TwitchBroadcasterClient::new(
                config.twitch_channel_to_join.clone().unwrap(),
                manager.clone(),
            )
        })
    }

    pub fn get_vrchat_osc(&self) -> Option<Arc<VRChatOSC>> {
        self.vrchat_osc.clone()
    }

    pub fn update_vrchat_status(&mut self, status: bool) {
        self.vrchat_status = status;
        let update_message = WebSocketMessage {
            message_type: "vrchat_status_update".to_string(),
            message: Some(status.to_string()),
            destination: None,
            world: None,
            additional_streams: None,
            user_id: None,
        };
        let _ = self.tx.send(update_message);
    }

    pub fn update_vrchat_world(&mut self, world: Option<World>) {
        println!("Updating VRChat world: {:?}", world);
        self.vrchat_world = world.clone();
        // Broadcast the world update to all connected clients
        if let Some(world) = &self.vrchat_world {
            let update_message = WebSocketMessage {
                message_type: "vrchat_world_update".to_string(),
                message: None,
                destination: None,
                world: Some(serde_json::to_value(world).unwrap()),
                additional_streams: None,
                user_id: None,
            };
            let _ = self.tx.send(update_message); // Ignore send errors
        }
    }

    pub fn update_twitch_status(&mut self, status: bool) {
        self.twitch_status = status;
    }

    pub fn update_discord_status(&mut self, status: bool) {
        self.discord_status = status;
    }

    pub fn update_obs_status(&mut self, status: bool) {
        self.obs_status = status;
    }
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
                "vrchat_world": dashboard_state.vrchat_world,
                "active_modules": ["twitch", "discord", "vrchat"]
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
                            match serde_json::from_str::<WebSocketMessage>(text) {
                                Ok(parsed_message) => {
                                    if let Err(e) = handle_ws_message(&parsed_message, &dashboard_state, &storage, &logger).await {
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
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut dashboard_state = dashboard_state.write().await;

    match message.message_type.as_str() {
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
            let response = WebSocketMessage {
                message_type: "worldShared".to_string(),
                message: Some("success".to_string()),
                destination: None,
                world: None,
                additional_streams: None,
                user_id: None,
            };
            dashboard_state.broadcast_message(response).await?;
        }
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

                let response = WebSocketMessage {
                    message_type: "chatSent".to_string(),
                    message: Some("success".to_string()),
                    destination: None,
                    world: None,
                    additional_streams: None,
                    user_id: None,
                };
                dashboard_state.broadcast_message(response).await?;
            }
        }
        "vrchat_world_update" => {
            if let Some(world) = &message.world {
                log_info!(logger, "Received VRChat world update: {:?}", world);
                if let Ok(world_data) = serde_json::from_value::<World>(world.clone()) {
                    dashboard_state.update_vrchat_world(Some(world_data));
                    dashboard_state.update_vrchat_status(true);
                } else {
                    log_error!(logger, "Failed to parse VRChat world data");
                }
            }
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
                dashboard_state.broadcast_message(broadcast_msg).await?;
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

pub async fn update_dashboard_state(
    state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    discord_client: Arc<RwLock<Option<Arc<DiscordClient>>>>,
    logger: Arc<Logger>,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

    loop {
        interval.tick().await;

        let update_message = {
            let state = state.read().await;
            let bot_status = state.bot_status.read().await;
            let discord_status = discord_client.read().await.is_some();
            let status = if bot_status.is_online() { "online" } else { "offline" };
            let uptime = bot_status.uptime_string();

            // Log the current VRChat world state
            log_info!(logger, "Current VRChat world state: {:?}", state.vrchat_world);

            // Fetch recent messages from storage
            let recent_messages = match storage.read().await.get_recent_messages(10).await {
                Ok(messages) => messages,
                Err(e) => {
                    log_error!(logger, "Failed to fetch recent messages: {:?}", e);
                    Vec::new()
                }
            };

            // Create the update message
            WebSocketMessage {
                message_type: "update".to_string(),
                message: Some(status.to_string()),
                destination: None,
                world: Some(serde_json::json!({
                    "uptime": uptime,
                    "vrchat_world": state.vrchat_world,
                    "recent_messages": recent_messages,
                    "twitch_status": state.twitch_status,
                    "discord_status": discord_status,
                    "vrchat_status": state.vrchat_status,
                    "obs_status": state.obs_status,
                })),
                additional_streams: None,
                user_id: None,
            }
        };

        // Broadcast the update
        let state = state.read().await;
        if let Err(e) = state.broadcast_message(update_message).await {
            log_error!(logger, "Failed to broadcast update message: {:?}", e);
        }
    }
}

// Helper function to create a new WebSocket connection
pub async fn create_websocket_connection(
    ws: WebSocket,
    dashboard_state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>,
) {
    tokio::spawn(handle_websocket(ws, dashboard_state, storage, logger));
}

// This function can be called from your main server setup to start the dashboard state update task
pub async fn start_dashboard_update_task(
    dashboard_state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    discord_client: Arc<RwLock<Option<Arc<DiscordClient>>>>,
    logger: Arc<Logger>,
) {
    tokio::spawn(update_dashboard_state(dashboard_state, storage, discord_client, logger));
}