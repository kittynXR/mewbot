use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use warp::ws::{Message, WebSocket};
use futures::{FutureExt, StreamExt};
use futures_util::SinkExt;
use serde_json::json;
use crate::config::Config;
use crate::vrchat::models::World;
use crate::twitch::irc::TwitchIRCClient;
use crate::storage::StorageClient;
use crate::bot_status::BotStatus;
use crate::osc::VRChatOSC;
use crate::web_ui::storage_ext::StorageClientExt;

pub struct DashboardState {
    pub(crate) bot_status: Arc<RwLock<BotStatus>>,
    pub(crate) vrchat_world: Option<World>,
    recent_messages: Vec<String>,
    config: Arc<RwLock<Config>>,
    twitch_client: Arc<TwitchIRCClient>,
    vrchat_osc: Option<Arc<VRChatOSC>>,
}

impl DashboardState {
    pub fn new(
        bot_status: Arc<RwLock<BotStatus>>,
        config: Arc<RwLock<Config>>,
        twitch_client: Arc<TwitchIRCClient>,
        vrchat_osc: Option<Arc<VRChatOSC>>,
    ) -> Self {
        Self {
            bot_status,
            vrchat_world: None,
            recent_messages: Vec::new(),
            config,
            twitch_client,
            vrchat_osc,
        }
    }

    pub async fn get_twitch_channel(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        self.config.read().await.twitch_channel_to_join
            .clone()
            .ok_or_else(|| "Twitch channel not set".into())
    }

    pub fn get_twitch_client(&self) -> Arc<TwitchIRCClient> {
        self.twitch_client.clone()
    }

    pub fn get_vrchat_osc(&self) -> Option<Arc<VRChatOSC>> {
        self.vrchat_osc.clone()
    }
}

pub async fn handle_ws_client(ws: WebSocket, state: Arc<RwLock<DashboardState>>, mut rx: broadcast::Receiver<()>) {
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
                            handle_ws_message(text, &state).await;
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("WebSocket error: {}", e);
                        break;
                    }
                    None => break,
                }
            }
        }
    }
}

async fn handle_ws_message(message: &str, state: &Arc<RwLock<DashboardState>>) {
    let msg: serde_json::Value = serde_json::from_str(message).unwrap();
    let state = state.read().await;

    match msg["type"].as_str() {
        Some("shareWorld") => {
            if let Some(world) = &state.vrchat_world {
                let world_info = format!("Current VRChat World: {} by {}", world.name, world.author_name);
                if let Ok(twitch_channel) = state.get_twitch_channel().await {
                    let twitch_client = state.get_twitch_client();
                    if let Err(e) = twitch_client.send_message(&twitch_channel, &world_info).await {
                        eprintln!("Error sending world info to Twitch chat: {:?}", e);
                    }
                }
            }
        }
        Some("sendChat") => {
            if let Some(chat_msg) = msg["message"].as_str() {
                let destinations = &msg["destination"];
                if destinations["oscTextbox"].as_bool().unwrap_or(false) {
                    if let Some(vrchat_osc) = state.get_vrchat_osc() {
                        if let Err(e) = vrchat_osc.send_chatbox_message(chat_msg, true, false) {
                            eprintln!("Error sending message to VRChat OSC: {:?}", e);
                        }
                    }
                }
                if destinations["twitchChat"].as_bool().unwrap_or(false) {
                    if let Ok(twitch_channel) = state.get_twitch_channel().await {
                        let twitch_client = state.get_twitch_client();
                        if let Err(e) = twitch_client.send_message(&twitch_channel, chat_msg).await {
                            eprintln!("Error sending message to Twitch chat: {:?}", e);
                        }
                    }
                }
            }
        }
        _ => {
            eprintln!("Unknown message type");
        }
    }
}

pub async fn update_dashboard_state(
    state: Arc<RwLock<DashboardState>>,
    tx: broadcast::Sender<()>,
    world_info: Arc<tokio::sync::Mutex<Option<World>>>,
    storage: Arc<RwLock<StorageClient>>,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));

    loop {
        interval.tick().await;

        let mut state = state.write().await;
        state.vrchat_world = world_info.lock().await.clone();

        // Fetch recent messages from storage
        if let Ok(messages) = storage.read().await.get_recent_messages(10).await {
            state.recent_messages = messages;
        }

        // Notify all connected clients about the state update
        let _ = tx.send(());
    }
}