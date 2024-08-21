use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use futures_util::{SinkExt, StreamExt};
use warp::ws::{Message, WebSocket};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use log::{error, info, debug, warn};
use tokio::sync::broadcast::error::SendError;
use crate::storage::StorageClient;
use crate::obs::OBSManager;
use crate::twitch::{TwitchIRCManager};
use crate::vrchat::{VRChatClient, VRChatManager};
use crate::config::Config;
use crate::bot_status::BotStatus;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WebSocketMessage {
    pub module: String,
    pub action: String,
    pub data: Value,
}

impl WebSocketMessage {
    pub fn new() -> Self {
        WebSocketMessage {
            module: String::new(),
            action: String::new(),
            data: serde_json::Value::Null,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Default)]
pub struct DashboardUpdateData {
    pub uptime: String,
    pub vrchat_world: Option<crate::vrchat::models::World>,
    pub recent_messages: Vec<String>,
    pub twitch_status: bool,
    pub discord_status: bool,
    pub vrchat_status: bool,
    pub obs_status: bool,
    pub obs_instances: Vec<crate::obs::OBSInstanceState>,
    pub instance_name: Option<String>,
    pub scene_name: Option<String>,
    pub source_name: Option<String>,
    pub enabled: Option<bool>,
}

pub struct DashboardState {
    pub(crate) bot_status: Arc<RwLock<BotStatus>>,
    pub(crate) vrchat_world: Option<crate::vrchat::models::World>,
    pub(crate) twitch_status: bool,
    pub(crate) discord_status: bool,
    pub(crate) vrchat_status: bool,
    pub(crate) obs_status: bool,
    recent_messages: Vec<String>,
    config: Arc<RwLock<Config>>,
    pub(crate) tx: broadcast::Sender<WebSocketMessage>,
    pub(crate) rx: broadcast::Receiver<WebSocketMessage>,
    pub obs_instances: Vec<crate::obs::OBSInstanceState>,
}

impl DashboardState {
    pub fn new(
        bot_status: Arc<RwLock<BotStatus>>,
        config: Arc<RwLock<Config>>,
    ) -> Self {
        let (tx, rx) = broadcast::channel(100); // You can adjust the channel size as needed
        Self {
            bot_status,
            vrchat_world: None,
            twitch_status: false,
            discord_status: false,
            vrchat_status: false,
            obs_status: false,
            recent_messages: Vec::new(),
            config,
            tx,
            rx,
            obs_instances: Vec::new(),
        }
    }
    pub fn update_twitch_status(&mut self, status: bool) {
        self.twitch_status = status;
        self.broadcast_update();
    }

    pub fn update_discord_status(&mut self, status: bool) {
        self.discord_status = status;
        self.broadcast_update();
    }

    pub fn update_vrchat_status(&mut self, status: bool) {
        self.vrchat_status = status;
        self.broadcast_update();
    }

    pub fn update_obs_status(&mut self, status: bool) {
        self.obs_status = status;
        self.broadcast_update();
    }

    pub fn update_vrchat_world(&mut self, world: Option<crate::vrchat::models::World>) {
        self.vrchat_world = world;
        self.broadcast_update();
    }

    async fn broadcast_update(&self) {
        let update = WebSocketMessage {
            module: "dashboard".to_string(),
            action: "update".to_string(),
            data: json!({
                "uptime": self.bot_status.read().await.uptime_string(),
                "vrchat_world": self.vrchat_world,
                "recent_messages": self.recent_messages,
                "twitch_status": self.twitch_status,
                "discord_status": self.discord_status,
                "vrchat_status": self.vrchat_status,
                "obs_status": self.obs_status,
                "obs_instances": self.obs_instances,
            }),
        };

        if let Err(e) = self.tx.send(update) {
            error!("Failed to broadcast dashboard update: {:?}", e);
        }
    }

    pub async fn broadcast_message(&self, message: WebSocketMessage) -> Result<usize, SendError<WebSocketMessage>> {
        self.tx.send(message)
    }
}

pub async fn handle_websocket(
    msg: WebSocketMessage,
    dashboard_state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    obs_manager: Arc<OBSManager>,
    twitch_irc_manager: Arc<TwitchIRCManager>,
    vrchat_manager: Arc<VRChatManager>,
) {
    // Remove the WebSocket splitting logic
    // let (mut ws_tx, mut ws_rx) = ws.split();

    // Instead, handle the WebSocketMessage directly
    match msg.module.as_str() {
        "obs" => {
            if let Err(e) = obs_manager.handle_message(msg).await {
                error!("Error handling OBS message: {:?}", e);
            }
        },
        "twitch" => {
            if let Err(e) = twitch_irc_manager.handle_message(msg).await {
                error!("Error handling Twitch message: {:?}", e);
            }
        },
        "vrchat" => {
            if let Err(e) = vrchat_manager.handle_message(msg).await {
                error!("Error handling VRChat message: {:?}", e);
            }
        },
        _ => {
            error!("Unknown module: {}", msg.module);
        }
    }

    // Remove the websocket loop logic
    // The function now handles a single message and returns
}

pub async fn send_dashboard_update(
    dashboard_state: &Arc<RwLock<DashboardState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let state = dashboard_state.read().await;
    let update = WebSocketMessage {
        module: "dashboard".to_string(),
        action: "update".to_string(),
        data: json!({
            "uptime": state.bot_status.read().await.uptime_string(),
            "vrchat_world": state.vrchat_world,
            "recent_messages": state.recent_messages,
            "twitch_status": state.twitch_status,
            "discord_status": state.discord_status,
            "vrchat_status": state.vrchat_status,
            "obs_status": state.obs_status,
            "obs_instances": state.obs_instances,
        }),
    };
    state.broadcast_message(update).await?;
    Ok(())
}

pub async fn update_dashboard_state(
    state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));

    info!("Starting dashboard state update loop");

    loop {
        tokio::select! {
            _ = interval.tick() => {
                debug!("Tick: Preparing to update dashboard state");
                if let Err(e) = send_dashboard_update(&state).await {
                    error!("Failed to send dashboard update: {:?}", e);
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

pub async fn start_dashboard_update_task(
    dashboard_state: Arc<RwLock<DashboardState>>,
    storage: Arc<RwLock<StorageClient>>,
) -> tokio::sync::oneshot::Sender<()> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    tokio::spawn(update_dashboard_state(
        dashboard_state,
        storage,
        shutdown_rx
    ));

    shutdown_tx
}