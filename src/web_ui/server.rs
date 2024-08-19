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
use crate::twitch::irc::TwitchIRCManager;
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
        twitch_irc_manager: Arc<TwitchIRCManager>,
        vrchat_osc: Option<Arc<VRChatOSC>>,
        discord_client: Option<Arc<DiscordClient>>,
        dashboard_state: Arc<RwLock<DashboardState>>,
    ) -> Self {
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

        let exe_path = env::current_exe().expect("Failed to get executable path");
        info!("Executable path: {:?}", exe_path);

        let project_root = exe_path.parent().unwrap().parent().unwrap().parent().unwrap();
        info!("Project root: {:?}", project_root);

        let build_path = project_root.join("web_ui").join("frontend").join("build");
        info!("Full path to build directory: {:?}", build_path);

        let build_path_str = build_path.to_str().unwrap().to_string();

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

        let catch_all = {
            let build_path_str = build_path_str.clone();
            warp::path("ui")
                .and(warp::path::tail())
                .and(warp::fs::file(format!("{}/index.html", build_path_str)))
                .map(|_, file| file)
        };

        let root_redirect = warp::path::end().map(|| {
            warp::redirect::see_other(warp::http::Uri::from_static("/ui/"))
        });

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

        let config_read = self.config.read().await;
        let host = config_read.web_ui_host.clone().unwrap_or_else(|| "127.0.0.1".to_string());
        let port = config_read.web_ui_port.unwrap_or(3333);
        drop(config_read);

        info!("Starting web UI server on {}:{}", host, port);

        let (_tx, _rx) = broadcast::channel::<WebSocketMessage>(100);

        let (update_task_shutdown_tx, update_task_shutdown_rx) = oneshot::channel();
        let update_task = tokio::spawn(update_dashboard_state(
            self.dashboard_state.clone(),
            self.storage.clone(),
            Arc::new(RwLock::new(self.discord_client.clone())),
            update_task_shutdown_rx,
        ));

        let addr: std::net::SocketAddr = format!("{}:{}", host, port).parse().expect("Invalid address");
        let (_, server) = warp::serve(routes).bind_with_graceful_shutdown(addr, shutdown_signal);

        let server_handle = tokio::spawn(server);

        server_handle.await?;

        warn!("Stopping dashboard update task...");
        if let Err(e) = update_task_shutdown_tx.send(()) {
            warn!("Failed to send shutdown signal to update task: {:?}", e);
        }

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