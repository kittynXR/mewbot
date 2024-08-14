use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use warp::ws::{Message, WebSocket};
use futures::StreamExt;
use futures_util::{AsyncWriteExt, SinkExt};
use serde_json::json;
use crate::config::Config;
use crate::vrchat::models::World;
use crate::twitch::irc::{TwitchIRCManager, TwitchBotClient, TwitchBroadcasterClient};
use crate::storage::StorageClient;
use crate::bot_status::BotStatus;
use crate::discord::DiscordClient;
use crate::{log_error, log_info, log_verbose};
use crate::LogLevel;
use crate::logging::Logger;
use crate::osc::VRChatOSC;
use crate::web_ui::storage_ext::StorageClientExt;
use crate::web_ui::websocket::WebSocketMessage;

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

pub async fn handle_ws_client(ws: WebSocket, state: Arc<RwLock<DashboardState>>, _storage: Arc<RwLock<StorageClient>>, logger: Arc<Logger>, mut rx: broadcast::Receiver<()>) {
    let (mut ws_tx, mut ws_rx) = ws.split();

    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));

    loop {
        tokio::select! {
            _ = interval.tick() => {
                let state = state.read().await;
                let bot_status = state.bot_status.read().await;
                let update = json!({
                    "type": "update",
                    "data": {
                        "bot_status": bot_status.is_online(),
                        "uptime": bot_status.uptime_string(),
                        "vrchat_world": state.vrchat_world,
                        "recent_messages": state.recent_messages,
                    }
                });
                if let Err(e) = ws_tx.send(Message::text(update.to_string())).await {
                    eprintln!("WebSocket send error: {}", e);
                    break;
                }
            }
            _ = rx.recv() => {
                // State has been updated, send an immediate update
                let state = state.read().await;
                let bot_status = state.bot_status.read().await;
                let update = json!({
                    "type": "update",
                    "data": {
                        "bot_status": bot_status.is_online(),
                        "uptime": bot_status.uptime_string(),
                        "vrchat_world": state.vrchat_world,
                        "recent_messages": state.recent_messages,
                    }
                });
                if let Err(e) = ws_tx.send(Message::text(update.to_string())).await {
                    eprintln!("WebSocket send error: {}", e);
                    break;
                }
            }
            msg = ws_rx.next() => {
                match msg {
                    Some(Ok(msg)) => {
                        if let Ok(text) = msg.to_str() {
                            if let Ok(parsed_message) = serde_json::from_str::<WebSocketMessage>(text) {
                                handle_ws_message(&parsed_message, &state, &logger).await;
                            } else {
                                log_error!(logger, "Failed to parse WebSocket message");
                            }
                        }
                    }
                    Some(Err(e)) => {
                        log_error!(logger, "WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                }
            }
        }
    }
}

async fn handle_ws_message(
    message: &WebSocketMessage,
    dashboard_state: &Arc<RwLock<DashboardState>>,
    logger: &Arc<Logger>
) {
    let mut dashboard_state = dashboard_state.write().await;

    match message.message_type.as_str() {
        "shareWorld" => {
            log_info!(logger, "Sharing VRChat world");
            if let Some(world) = &dashboard_state.vrchat_world {
                let world_info = format!("Current VRChat World: {} by {}", world.name, world.author_name);
                if let Ok(twitch_channel) = dashboard_state.get_twitch_channel().await {
                    if let Some(twitch_bot_client) = dashboard_state.get_twitch_bot_client().await {
                        if let Err(e) = twitch_bot_client.send_message(&twitch_channel, &world_info).await {
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
                user_id: None, // Add this line
            };
            if let Err(e) = dashboard_state.tx.send(response) {
                log_error!(logger, "Failed to broadcast world shared status: {:?}", e);
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
                            if let Some(twitch_bot_client) = dashboard_state.get_twitch_bot_client().await {
                                if let Err(e) = twitch_bot_client.send_message(&twitch_channel, chat_msg).await {
                                    log_error!(logger, "Error sending message to Twitch chat: {:?}", e);
                                }
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
                    user_id: None, // Add this line
                };
                if let Err(e) = dashboard_state.tx.send(response) {
                    log_error!(logger, "Failed to broadcast chat sent status: {:?}", e);
                }
            } else {
                let error_response = WebSocketMessage {
                    message_type: "error".to_string(),
                    message: Some("Invalid chat message".to_string()),
                    destination: None,
                    world: None,
                    additional_streams: None,
                    user_id: None, // Add this line
                };
                if let Err(e) = dashboard_state.tx.send(error_response) {
                    log_error!(logger, "Failed to broadcast error message: {:?}", e);
                }
            }
        }
        "twitch_message" => {
            if let (Some(chat_msg), Some(user_id)) = (&message.message, &message.user_id) {
                log_info!(logger, "Received Twitch message from {}: {}", user_id, chat_msg);
                let broadcast_msg = WebSocketMessage {
                    message_type: "twitch_message".to_string(),
                    message: Some(chat_msg.clone()),
                    user_id: Some(user_id.clone()),
                    destination: None,
                    world: None,
                    additional_streams: None,
                };
                if let Err(e) = dashboard_state.tx.send(broadcast_msg) {
                    log_error!(logger, "Failed to broadcast Twitch message: {:?}", e);
                }
            }
        }
        _ => {
            log_error!(logger, "Unknown message type: {}", message.message_type);
        }
    }
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