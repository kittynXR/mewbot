use std::env;
use std::future::Future;
use warp::Filter;
use std::sync::Arc;
use log::{info, warn};
use tokio::sync::{broadcast, oneshot, RwLock};
use tokio::task::JoinHandle;
use warp::http::{HeaderMap, HeaderValue};
use crate::bot_status::BotStatus;
use crate::config::Config;
use crate::storage::StorageClient;
use crate::osc::VRChatOSC;
use crate::twitch::irc::{TwitchIRCManager, TwitchBotClient}; // Updated import
use crate::web_ui::websocket;
use crate::web_ui::websocket::{DashboardState, update_dashboard_state};
use super::websocket::{handle_websocket, WebSocketMessage};
use super::api_routes::{api_routes, with_storage};
use crate::discord::DiscordClient;

pub struct WebUI {
    config: Arc<RwLock<Config>>,
    storage: Arc<RwLock<StorageClient>>,
    pub dashboard_state: Arc<RwLock<websocket::DashboardState>>,
    discord_client: Option<Arc<DiscordClient>>,
}

impl WebUI {
    pub fn new(
        config: Arc<RwLock<Config>>,
        storage: Arc<RwLock<StorageClient>>,
        bot_status: Arc<RwLock<BotStatus>>,
        twitch_irc_manager: Arc<TwitchIRCManager>, // Updated parameter
        vrchat_osc: Option<Arc<VRChatOSC>>,
        discord_client: Option<Arc<DiscordClient>>,
    ) -> Self {
        let dashboard_state = Arc::new(RwLock::new(DashboardState::new(
            bot_status,
            config.clone(),
            Some(twitch_irc_manager), // Pass TwitchIRCManager instead of TwitchIRCClient
            vrchat_osc,
        )));

        WebUI {
            config,
            storage,
            dashboard_state,
            discord_client,
        }
    }

    pub async fn run(&self, shutdown_signal: impl Future<Output = ()> + Send + 'static) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.clone();
        let storage = self.storage.clone();
        let dashboard_state = self.dashboard_state.clone();

        // Get the path of the current executable
        let exe_path = env::current_exe().expect("Failed to get executable path");
        info!("Executable path: {:?}", exe_path);

        // Navigate to the project root (assuming the executable is in target/debug or target/release)
        let project_root = exe_path.parent().unwrap().parent().unwrap().parent().unwrap();
        info!("Project root: {:?}", project_root);

        // Construct the path to the build directory
        let build_path = project_root.join("web_ui").join("frontend").join("build");
        info!("Full path to build directory: {:?}", build_path);

        // Convert build_path to a String
        let build_path_str = build_path.to_str().unwrap().to_string();

        // Serve React app
        let static_files = {
            let build_path_str = build_path_str.clone();
            warp::path("ui").and(warp::fs::dir(build_path_str.clone()))
                .or(warp::path("static").and(warp::fs::dir(format!("{}/static", build_path_str))))
                .with(warp::log::custom(move |info| {
                    info!("Static file request: {} {} {}",
                        info.method(),
                        info.path(),
                        info.status().as_u16()
                    );
                }))
        };

        // Catch-all route for React routing
        let catch_all = {
            let build_path_str = build_path_str.clone();
            warp::path("ui")
                .and(warp::path::tail())
                .and(warp::fs::file(format!("{}/index.html", build_path_str)))
                .map(|_, file| file)
        };

        // Redirect root to /ui/
        let root_redirect = warp::path::end().map(|| {
            warp::redirect::see_other(warp::http::Uri::from_static("/ui/"))
        });

        // WebSocket route
        let ws_route = warp::path("ws")
            .and(warp::ws())
            .and(with_dashboard_state(dashboard_state.clone()))
            .and(with_storage(storage.clone()))
            .map(|ws: warp::ws::Ws, dashboard_state, storage| {
                ws.on_upgrade(move |socket| handle_websocket(socket, dashboard_state, storage))
            });

        let api = api_routes(
            config.clone(),
            storage.clone(),
            dashboard_state.clone()
        );

        let routes = {
            root_redirect
                .or(static_files)
                .or(catch_all)
                .or(ws_route)
                .or(api)
                .with(warp::log::custom(move |info| {
                    info!("Request: {} {} {}",
                        info.method(),
                        info.path(),
                        info.status().as_u16()
                    );
                }))
                .with(warp::reply::with::headers(header_map()))
        };

        // Get the host and port from config, or use defaults
        let config_read = self.config.read().await;
        let host = config_read.web_ui_host.clone().unwrap_or_else(|| "127.0.0.1".to_string());
        let port = config_read.web_ui_port.unwrap_or(3333);
        drop(config_read);

        info!("Starting web UI server on {}:{}", host, port);

        // Create a broadcast channel for WebSocket messages
        let (_tx, _rx) = broadcast::channel::<WebSocketMessage>(100);

        // Start the periodic update task
        let (update_task_shutdown_tx, update_task_shutdown_rx) = oneshot::channel();
        let update_task = tokio::spawn(update_dashboard_state(
            self.dashboard_state.clone(),
            self.storage.clone(),
            Arc::new(RwLock::new(self.discord_client.clone())),
            update_task_shutdown_rx,
        ));

        // Run the server
        let addr: std::net::SocketAddr = format!("{}:{}", host, port).parse().expect("Invalid address");
        let (_, server) = warp::serve(routes).bind_with_graceful_shutdown(addr, shutdown_signal);

        // Run the server in a separate task
        let server_handle = tokio::spawn(server);

        // Wait for the server to complete (this will happen when shutdown_signal is triggered)
        server_handle.await?;

        // Stop the update task
        warn!("Stopping dashboard update task...");
        if let Err(e) = update_task_shutdown_tx.send(()) {
            warn!("Failed to send shutdown signal to update task: {:?}", e);
        }

        // Wait for the update task to finish
        match update_task.await {
            Ok(_) => info!("Dashboard update task stopped successfully"),
            Err(e) => warn!("Error while stopping dashboard update task: {:?}", e),
        }

        info!("Web UI server has shut down.");
        Ok(())
    }
}

fn header_map() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Security-Policy",
                   HeaderValue::from_static("frame-ancestors 'self' https://player.twitch.tv http://player.twitch.tv"));
    headers
}

fn with_dashboard_state(
    dashboard_state: Arc<RwLock<DashboardState>>,
) -> impl Filter<Extract = (Arc<RwLock<DashboardState>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || dashboard_state.clone())
}