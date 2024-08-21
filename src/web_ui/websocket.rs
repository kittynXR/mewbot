use std::cmp::PartialEq;
use std::sync::Arc;
use std::time::Duration;
use async_trait::async_trait;
use tokio::sync::{broadcast, oneshot, RwLock};
use warp::ws::{Message, WebSocket};
use futures::{StreamExt, SinkExt};
use log::{debug, error, info, trace, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::io::AsyncWriteExt;
use crate::config::Config;
use crate::vrchat::models::World;
use crate::twitch::irc::{TwitchIRCManager, TwitchBotClient, TwitchBroadcasterClient};
use crate::storage::StorageClient;
use crate::bot_status::BotStatus;
use crate::discord::DiscordClient;
use crate::osc::VRChatOSC;
use crate::web_ui::storage_ext::StorageClientExt;
use crate::obs::{OBSManager, OBSStateUpdate};
use crate::obs::models::OBSInstance as OBSModelInstance;
use crate::obs::OBSInstanceState;


#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatDestination {
    #[serde(rename = "oscTextbox", alias = "osc_textbox")]
    pub osc_textbox: bool,
    #[serde(rename = "twitchChat", alias = "twitch_chat")]
    pub twitch_chat: bool,
    #[serde(rename = "twitchBot", alias = "twitch_bot")]
    pub twitch_bot: bool,
    #[serde(rename = "twitchBroadcaster", alias = "twitch_broadcaster")]
    pub twitch_broadcaster: bool,
}

// New struct for dashboard update data
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct DashboardUpdateData {
    pub uptime: String,
    pub vrchat_world: Option<World>,
    pub recent_messages: Vec<String>,
    pub twitch_status: bool,
    pub discord_status: bool,
    pub vrchat_status: bool,
    pub obs_status: bool,
    pub obs_instances: Vec<OBSInstanceState>,
    // Fields for OBS operations
    pub instance_name: Option<String>,
    pub scene_name: Option<String>,
    pub source_name: Option<String>,
    pub enabled: Option<bool>,
}


// Modified WebSocketMessage struct
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct WebSocketMessage {
    #[serde(rename = "type")]
    pub message_type: String,
    pub message: Option<String>,
    pub destination: Option<ChatDestination>,
    pub update_data: Option<DashboardUpdateData>,
    #[serde(rename = "additionalStreams", alias = "additional_streams")]
    pub additional_streams: Option<Vec<String>>,
    pub user_id: Option<String>,
}

impl PartialEq for WebSocketMessage {
    fn eq(&self, other: &Self) -> bool {
        self.message_type == other.message_type
            && self.message == other.message
            && self.destination == other.destination
            && self.update_data == other.update_data
            && self.additional_streams == other.additional_streams
            && self.user_id == other.user_id
    }
}

pub struct WorldState {
    current_world: Option<World>,
    last_updated: std::time::Instant,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OBSInstance {
    pub id: u32,
    pub name: String,
    pub scenes: Vec<String>,
    pub current_scene: String,
    pub sources: std::collections::HashMap<String, Vec<OBSSource>>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct OBSSource {
    pub name: String,
    pub type_: String,
    pub enabled: bool,
}

#[async_trait]
impl OBSStateUpdate for Arc<RwLock<DashboardState>> {
    async fn update_obs_state(&self, instances: Vec<OBSInstanceState>) {
        let mut state = self.write().await;
        state.obs_instances = instances;
    }
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
    obs_manager: Option<Arc<OBSManager>>,
    pub obs_instances: Vec<OBSInstanceState>,
}

impl DashboardState {
    pub fn new(
        bot_status: Arc<RwLock<BotStatus>>,
        config: Arc<RwLock<Config>>,
        twitch_irc_manager: Option<Arc<TwitchIRCManager>>,
        vrchat_osc: Option<Arc<VRChatOSC>>,
        obs_manager: Option<Arc<OBSManager>>,
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
            obs_manager,
            obs_instances: Vec::new(),
        }
    }

    pub async fn broadcast_message(&self, message: WebSocketMessage) -> Result<usize, broadcast::error::SendError<WebSocketMessage>> {
        self.tx.send(message)
    }

    pub fn set_obs_manager(&mut self, manager: Arc<OBSManager>) {
        self.obs_manager = Some(manager);
    }

    pub fn set_twitch_irc_manager(&mut self, manager: Option<Arc<TwitchIRCManager>>) {
        self.twitch_irc_manager = manager.clone();
        self.twitch_status = manager.is_some();
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
    pub async fn update_obs_instances(&mut self) {
        if let Some(obs_manager) = &self.obs_manager {
            self.obs_instances = obs_manager.get_instances().await;
        } else {
            warn!("Attempted to update OBS instances, but OBS manager is not initialized.");
        }
    }
}

pub async fn handle_websocket(
    ws: WebSocket,
    dashboard_state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
) {
    let (mut ws_tx, mut ws_rx) = ws.split();

    // Implement a handshake
    if let Err(e) = ws_tx.send(Message::text("READY")).await {
        error!("Failed to send handshake: {:?}", e);
        return;
    }

    // Wait for client acknowledgment
    match ws_rx.next().await {
        Some(Ok(msg)) if msg.to_str() == Ok("ACK") => {},
        _ => {
            error!("Failed to receive handshake acknowledgment");
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
                                if let Err(e) = handle_ws_message(&parsed_message, &dashboard_state, &storage).await {
                                    error!("Error handling WebSocket message: {:?}", e);
                                    break;
                                }
                            } else {
                                error!("Failed to parse WebSocket message");
                                warn!("Received WebSocket message: {}", text);
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!("WebSocket error: {:?}", e);
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
                                error!("Failed to send WebSocket message: {:?}", e);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        error!("Failed to receive broadcast message: {:?}", e);
                        break;
                    }
                }
            }
            _ = ping_interval.tick() => {
                if let Err(e) = ws_tx.send(Message::ping(vec![])).await {
                    error!("Failed to send ping: {:?}", e);
                    break;
                }
            }
        }
    }
    info!("WebSocket connection closed");
}

async fn handle_ws_message(
    message: &WebSocketMessage,
    dashboard_state: &Arc<RwLock<DashboardState>>,
    storage: &Arc<RwLock<StorageClient>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match message.message_type.as_str() {
        "shareWorld" => {
            let world_info = {
                let state = dashboard_state.read().await;
                state.vrchat_world.as_ref().map(|world| {
                    (
                        format!(
                            "Current World: {} | Author: {} | Capacity: {} | Description: {} | Status: {}",
                            world.name, world.author_name, world.capacity, world.description, world.release_status
                        ),
                        format!(
                            "Published: {} | Last Updated: {} | World Link: https://vrchat.com/home/world/{}",
                            world.created_at.format("%Y-%m-%d"),
                            world.updated_at.format("%Y-%m-%d"),
                            world.id
                        )
                    )
                })
            };

            if let Some((first_message, second_message)) = world_info {
                let twitch_channel = dashboard_state.read().await.get_twitch_channel().await?;
                if let Some(twitch_client) = dashboard_state.read().await.get_twitch_bot_client().await {
                    twitch_client.send_message(&twitch_channel, &first_message).await?;
                    twitch_client.send_message(&twitch_channel, &second_message).await?;
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
            if let Some(update_data) = &message.update_data {
                if let Some(world_data) = &update_data.vrchat_world {
                    {
                        let mut state = dashboard_state.write().await;
                        state.update_vrchat_world(Some(world_data.clone()));
                        state.update_vrchat_status(true);
                    }

                    let broadcast_msg = WebSocketMessage {
                        message_type: "vrchat_world_update".to_string(),
                        update_data: Some(DashboardUpdateData {
                            vrchat_world: Some(world_data.clone()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    };
                    dashboard_state.read().await.broadcast_message(broadcast_msg).await?;
                } else {
                    error!("VRChat world data not found in update_data");
                }
            } else {
                error!("update_data not found in vrchat_world_update message");
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
                error!("Received incomplete Twitch message");
            }
        }
        "get_obs_info" | "change_scene" | "toggle_source" | "refresh_source" => {
            let mut state = dashboard_state.write().await;
            state.update_obs_instances().await;

            let response = WebSocketMessage {
                message_type: "obs_update".to_string(),
                update_data: Some(DashboardUpdateData {
                    uptime: state.bot_status.read().await.uptime_string(),
                    vrchat_world: state.vrchat_world.clone(),
                    recent_messages: Vec::new(), // You might want to populate this from somewhere
                    twitch_status: state.twitch_status,
                    discord_status: state.discord_status,
                    vrchat_status: state.vrchat_status,
                    obs_status: state.obs_status,
                    obs_instances: state.obs_instances.clone(),
                    instance_name: None,
                    scene_name: None,
                    source_name: None,
                    enabled: None,
                }),
                ..Default::default()
            };
            state.broadcast_message(response).await?;
        }
        _ => {
            error!("Unknown message type: {}", message.message_type);
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

pub async fn update_dashboard_state(
    state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    discord_client: Arc<RwLock<Option<Arc<DiscordClient>>>>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));
    let mut last_update: Option<WebSocketMessage> = None;

    info!("Starting dashboard state update loop");

    loop {
        tokio::select! {
            _ = interval.tick() => {
                debug!("Tick: Preparing to update dashboard state");
                let update_message = {
                    let state = state.read().await;
                    let bot_status = state.bot_status.read().await;
                    let discord_status = discord_client.read().await.is_some();
                    let status = if bot_status.is_online() { "online" } else { "offline" };
                    let uptime = bot_status.uptime_string();

                    debug!("Current VRChat world state (update dashboard): {:?}", state.vrchat_world);
                    trace!("Bot status: {}, Uptime: {}", status, uptime);

                    let recent_messages = match storage.read().await.get_recent_messages(10).await {
                        Ok(messages) => messages,
                        Err(e) => {
                            error!("Failed to fetch recent messages: {:?}", e);
                            warn!("Using empty vector for recent messages due to fetch failure");
                            Vec::new()
                        }
                    };

                    debug!("Fetched {} recent messages", recent_messages.len());

                    let update_data = DashboardUpdateData {
                        uptime,
                        vrchat_world: state.vrchat_world.clone(),
                        recent_messages,
                        twitch_status: state.twitch_status,
                        discord_status,
                        vrchat_status: state.vrchat_status,
                        obs_status: state.obs_status,
                        obs_instances: state.obs_instances.clone(),
                        instance_name: None,
                        scene_name: None,
                        source_name: None,
                        enabled: None,
                    };

                    trace!("Prepared update data: Discord status: {}, VRChat status: {}, OBS status: {}",
                          discord_status, state.vrchat_status, state.obs_status);

                    WebSocketMessage {
                        message_type: "update".to_string(),
                        message: Some(status.to_string()),
                        destination: None,
                        update_data: Some(update_data),
                        additional_streams: None,
                        user_id: None,
                    }
                };

                if last_update.as_ref() != Some(&update_message) {
                    let state = state.read().await;
                    warn!("Full WebSocketMessage to be sent: {:?}", update_message);
                    if let Err(e) = state.broadcast_message(update_message.clone()).await {
                        error!("Failed to broadcast update message: {:?}", e);
                    } else {
                        trace!("Successfully broadcasted update message");
                        last_update = Some(update_message);
                    }
                } else {
                    trace!("No changes detected, skipping update broadcast");
                }
            }
            _ = &mut shutdown_rx => {
                warn!("Received shutdown signal, stopping dashboard updates.");
                break;
            }
        }
    }

    warn!("Dashboard update task has stopped.");
}

// This function can be called from your main server setup to start the dashboard state update task
pub async fn start_dashboard_update_task(
    dashboard_state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    discord_client: Arc<RwLock<Option<Arc<DiscordClient>>>>,
) -> oneshot::Sender<()> {
    let (shutdown_tx, shutdown_rx) = oneshot::channel();

    tokio::spawn(update_dashboard_state(
        dashboard_state,
        storage,
        discord_client,
        shutdown_rx
    ));

    shutdown_tx
}

// Helper function to create a new WebSocket connection
pub async fn create_websocket_connection(
    ws: WebSocket,
    dashboard_state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
) {
    tokio::spawn(handle_websocket(ws, dashboard_state, storage));
}