use std::cmp::PartialEq;
use std::sync::Arc;
use std::time::Duration;
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
#[derive(Default)]
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
    pub osc_textbox: bool,
    pub twitch_chat: bool,
    pub twitch_bot: bool,
    pub twitch_broadcaster: bool,
}

pub struct WorldState {
    current_world: Option<World>,
    last_updated: std::time::Instant,
}

impl WorldState {
    pub fn new() -> Self {
        Self {
            current_world: None,
            last_updated: std::time::Instant::now(),
        }
    }

    pub fn update(&mut self, new_world: Option<World>) {
        self.current_world = new_world;
        self.last_updated = std::time::Instant::now();
    }

    pub fn get(&self) -> Option<World> {
        self.current_world.clone()
    }

    pub fn is_stale(&self, threshold: std::time::Duration) -> bool {
        self.last_updated.elapsed() > threshold
    }
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
    world_state: Arc<RwLock<WorldState>>,
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
            world_state: Arc::new(RwLock::new(WorldState::new())),
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
    }

    pub fn update_vrchat_world(&mut self, world: Option<World>) {
        self.vrchat_world = world;
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

    // Implement a handshake
    if let Err(e) = ws_tx.send(Message::text("READY")).await {
        log_error!(logger, "Failed to send handshake: {:?}", e);
        return;
    }

    // Wait for client acknowledgment
    match ws_rx.next().await {
        Some(Ok(msg)) if msg.to_str() == Ok("ACK") => {},
        _ => {
            log_error!(logger, "Failed to receive handshake acknowledgment");
            return;
        }
    }

    // Create a new receiver for this connection
    let mut rx = dashboard_state.read().await.tx.subscribe();

    let mut ping_interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(msg)) => {
                        if let Ok(text) = msg.to_str() {
                            if let Ok(parsed_message) = serde_json::from_str::<WebSocketMessage>(text) {
                                if let Err(e) = handle_ws_message(&parsed_message, &dashboard_state, &storage, &logger).await {
                                    log_error!(logger, "Error handling WebSocket message: {:?}", e);
                                    break;
                                }
                            } else {
                                log_error!(logger, "Failed to parse WebSocket message");
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
            _ = ping_interval.tick() => {
                if let Err(e) = ws_tx.send(Message::ping(vec![])).await {
                    log_error!(logger, "Failed to send ping: {:?}", e);
                    break;
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
    match message.message_type.as_str() {
        "shareWorld" => {
            let world_info = {
                let state = dashboard_state.read().await;
                state.vrchat_world.as_ref().map(|world| {
                    format!("Current VRChat World: {} by {}", world.name, world.author_name)
                })
            };

            if let Some(world_info) = world_info {
                let twitch_channel = dashboard_state.read().await.get_twitch_channel().await?;
                if let Some(twitch_client) = dashboard_state.read().await.get_twitch_bot_client().await {
                    twitch_client.send_message(&twitch_channel, &world_info).await?;
                }
            }

            let response = WebSocketMessage {
                message_type: "worldShared".to_string(),
                message: Some("success".to_string()),
                ..Default::default()
            };
            dashboard_state.read().await.broadcast_message(response).await?;
        }
        "sendChat" => {
            if let Some(chat_msg) = &message.message {
                let destinations = message.destination.clone();
                let additional_streams = message.additional_streams.clone();

                // Handle VRChat OSC
                if destinations.as_ref().map_or(false, |d| d.osc_textbox) {
                    if let Some(vrchat_osc) = dashboard_state.read().await.get_vrchat_osc() {
                        vrchat_osc.send_chatbox_message(chat_msg, true, false)?;
                    }
                }

                // Handle Twitch chat
                if destinations.as_ref().map_or(false, |d| d.twitch_chat) {
                    let twitch_channel = dashboard_state.read().await.get_twitch_channel().await?;

                    if destinations.as_ref().map_or(false, |d| d.twitch_bot) {
                        if let Some(twitch_client) = dashboard_state.read().await.get_twitch_bot_client().await {
                            twitch_client.send_message(&twitch_channel, chat_msg).await?;
                        }
                    }

                    if destinations.as_ref().map_or(false, |d| d.twitch_broadcaster) {
                        if let Some(twitch_client) = dashboard_state.read().await.get_twitch_broadcaster_client().await {
                            twitch_client.send_message(&twitch_channel, chat_msg).await?;
                        }
                    }
                }

                // Handle additional streams
                if let Some(streams) = additional_streams {
                    if let Some(twitch_client) = dashboard_state.read().await.get_twitch_bot_client().await {
                        for stream in streams {
                            twitch_client.send_message(&stream, chat_msg).await?;
                        }
                    }
                }

                let response = WebSocketMessage {
                    message_type: "chatSent".to_string(),
                    message: Some("success".to_string()),
                    ..Default::default()
                };
                dashboard_state.read().await.broadcast_message(response).await?;
            }
        }
        "vrchat_world_update" => {
            if let Some(world) = &message.world {
                if let Ok(world_data) = serde_json::from_value::<World>(world.clone()) {
                    {
                        let mut state = dashboard_state.write().await;
                        state.update_vrchat_world(Some(world_data.clone()));
                        state.update_vrchat_status(true);
                    }

                    let broadcast_msg = WebSocketMessage {
                        message_type: "vrchat_world_update".to_string(),
                        world: Some(serde_json::to_value(&world_data)?),
                        ..Default::default()
                    };
                    dashboard_state.read().await.broadcast_message(broadcast_msg).await?;
                } else {
                    log_error!(logger, "Failed to parse VRChat world data");
                }
            }
        }
        "twitch_message" => {
            if let (Some(chat_msg), Some(user_id)) = (&message.message, &message.user_id) {
                storage.write().await.add_message(user_id, chat_msg)?;

                let broadcast_msg = WebSocketMessage {
                    message_type: "twitch_message".to_string(),
                    message: Some(chat_msg.clone()),
                    user_id: Some(user_id.clone()),
                    ..Default::default()
                };
                dashboard_state.read().await.broadcast_message(broadcast_msg).await?;
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

impl PartialEq for World {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.name == other.name
    }
}

impl PartialEq for ChatDestination {
    fn eq(&self, other: &Self) -> bool {
        self.osc_textbox == other.osc_textbox
            && self.twitch_chat == other.twitch_chat
            && self.twitch_bot == other.twitch_bot
            && self.twitch_broadcaster == other.twitch_broadcaster
    }
}

impl PartialEq for WebSocketMessage {
    fn eq(&self, other: &Self) -> bool {
        self.message_type == other.message_type
            && self.message == other.message
            && self.destination == other.destination
            && self.world == other.world
            && self.additional_streams == other.additional_streams
            && self.user_id == other.user_id
    }
}

pub async fn update_dashboard_state(
    state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    discord_client: Arc<RwLock<Option<Arc<DiscordClient>>>>,
    logger: Arc<Logger>,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));
    let mut last_update: Option<WebSocketMessage> = None;

    loop {
        interval.tick().await;

        let update_message = {
            let state = state.read().await;
            let bot_status = state.bot_status.read().await;
            let discord_status = discord_client.read().await.is_some();
            let status = if bot_status.is_online() { "online" } else { "offline" };
            let uptime = bot_status.uptime_string();

            let world_state = state.world_state.read().await;
            let current_world = world_state.get();

            log_info!(logger, "Current VRChat world state (update dashboard): {:?}", current_world);

            let recent_messages = match storage.read().await.get_recent_messages(10).await {
                Ok(messages) => messages,
                Err(e) => {
                    log_error!(logger, "Failed to fetch recent messages: {:?}", e);
                    Vec::new()
                }
            };

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

        if last_update.as_ref() != Some(&update_message) {
            let state = state.read().await;
            if let Err(e) = state.broadcast_message(update_message.clone()).await {
                log_error!(logger, "Failed to broadcast update message: {:?}", e);
            } else {
                last_update = Some(update_message);
            }
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