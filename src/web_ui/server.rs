use std::env;
use std::future::Future;
use warp::Filter;
use std::sync::Arc;
use std::net::{IpAddr, SocketAddr, TcpListener};
use futures_util::StreamExt;
use log::{error, info, warn};
use tokio::sync::{broadcast, oneshot, RwLock};
use warp::http::{HeaderMap, HeaderValue};
use warp::ws::WebSocket;
use crate::bot_status::BotStatus;
use crate::config::Config;
use crate::storage::StorageClient;
use crate::twitch::irc::TwitchIRCManager;
use crate::web_ui::websocket;
use crate::web_ui::websocket::{DashboardState, update_dashboard_state};
use super::websocket::{handle_websocket, WebSocketMessage};
use super::api_routes::{api_routes};
use crate::obs::OBSManager;
use crate::vrchat::VRChatManager;

pub struct WebUI {
    config: Arc<RwLock<Config>>,
    storage: Arc<RwLock<StorageClient>>,
    pub dashboard_state: Arc<RwLock<websocket::DashboardState>>,
    obs_manager: Arc<OBSManager>,
    twitch_irc_manager: Arc<TwitchIRCManager>,
    vrchat_manager: Arc<VRChatManager>,
}

impl WebUI {
    pub fn new(
        config: Arc<RwLock<Config>>,
        storage: Arc<RwLock<StorageClient>>,
        _bot_status: Arc<RwLock<BotStatus>>,
        twitch_irc_manager: Arc<TwitchIRCManager>,
        dashboard_state: Arc<RwLock<DashboardState>>,
        obs_manager: Arc<OBSManager>,
        vrchat_manager: Arc<VRChatManager>,
    ) -> Self {
        WebUI {
            config,
            storage,
            dashboard_state,
            obs_manager,
            twitch_irc_manager,
            vrchat_manager,
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
            .and(with_obs_manager(self.obs_manager.clone()))
            .and(with_twitch_irc_manager(self.twitch_irc_manager.clone()))
            .and(with_vrchat_manager(self.vrchat_manager.clone()))
            .map(|ws: warp::ws::Ws, obs_manager, twitch_irc_manager, vrchat_manager| {
                ws.on_upgrade(move |socket| {
                    handle_ws_connection(socket, obs_manager, twitch_irc_manager, vrchat_manager)
                })
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
        let (host, port) = self.parse_address_config(&config_read)?;
        drop(config_read);

        info!("Starting web UI server on {}:{}", host, port);

        let (_tx, _rx) = broadcast::channel::<WebSocketMessage>(100);

        let (update_task_shutdown_tx, update_task_shutdown_rx) = oneshot::channel();
        let update_task = tokio::spawn(update_dashboard_state(
            self.dashboard_state.clone(),
            update_task_shutdown_rx,
        ));

        let addr = SocketAddr::new(host, port);
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

    fn is_port_available(host: IpAddr, port: u16) -> bool {
        let addr = SocketAddr::new(host, port);
        TcpListener::bind(addr).is_ok()
    }

    fn find_available_port(host: IpAddr, preferred_port: u16) -> Option<u16> {
        if Self::is_port_available(host, preferred_port) {
            return Some(preferred_port);
        }

        // Try the next 10 ports
        for port in (preferred_port + 1)..=(preferred_port + 10) {
            if Self::is_port_available(host, port) {
                return Some(port);
            }
        }

        None
    }

    fn parse_address_config(&self, config: &Config) -> Result<(IpAddr, u16), Box<dyn std::error::Error + Send + Sync>> {
        let host_str = config.web_ui_host.clone().unwrap_or_else(|| "127.0.0.1".to_string());
        let preferred_port = config.web_ui_port.unwrap_or(3333);

        // Validate port
        if !(1..=65535).contains(&preferred_port) {
            return Err(format!("Invalid port number: {}. Port must be between 1 and 65535", preferred_port).into());
        }

        // Parse and validate host
        let ip_addr = match host_str.as_str() {
            "localhost" => Ok::<IpAddr, std::net::AddrParseError>("127.0.0.1".parse::<IpAddr>()?),
            host => {
                match host.parse::<IpAddr>() {
                    Ok(ip) => Ok(ip),
                    Err(_) => {
                        warn!("Invalid host address '{}', falling back to 127.0.0.1", host);
                        Ok("127.0.0.1".parse::<IpAddr>()?)
                    }
                }
            }
        }?;

        // Additional validation for unroutable addresses
        let ip_addr = if ip_addr.is_unspecified() {
            warn!("Unspecified address detected, using 127.0.0.1 instead");
            "127.0.0.1".parse::<IpAddr>()?
        } else if ip_addr.is_multicast() {
            return Err("Multicast addresses are not supported".into());
        } else {
            ip_addr
        };

        // Find an available port
        let port = Self::find_available_port(ip_addr, preferred_port)
            .ok_or_else(|| {
                format!(
                    "Unable to bind to ports {}-{}. Please ensure no other instance is running and try again.",
                    preferred_port,
                    preferred_port + 10
                )
            })?;

        if port != preferred_port {
            warn!("Preferred port {} was in use, using port {} instead", preferred_port, port);
        }

        Ok((ip_addr, port))
    }
}

async fn handle_ws_connection(
    ws: WebSocket,
    obs_manager: Arc<OBSManager>,
    twitch_irc_manager: Arc<TwitchIRCManager>,
    vrchat_manager: Arc<VRChatManager>,
) {
    let (_, mut ws_recv) = ws.split();

    while let Some(result) = ws_recv.next().await {
        match result {
            Ok(msg) => {
                warn!("received websocket text {:?}", msg.to_str());

                if let Ok(text) = msg.to_str() {
                    if let Ok(ws_msg) = serde_json::from_str::<WebSocketMessage>(text) {
                        handle_websocket(ws_msg, obs_manager.clone(), twitch_irc_manager.clone(), vrchat_manager.clone()).await;
                    } else {
                        error!("Failed to parse WebSocket message: {}", text);
                    }
                }
            }
            Err(e) => {
                error!("WebSocket error: {:?}", e);
                break;
            }
        }
    }
}

fn header_map() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("Content-Security-Policy",
                   HeaderValue::from_static("frame-ancestors 'self' https://player.twitch.tv http://player.twitch.tv"));
    headers
}

fn with_obs_manager(
    obs_manager: Arc<OBSManager>,
) -> impl Filter<Extract = (Arc<OBSManager>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || obs_manager.clone())
}

fn with_twitch_irc_manager(
    twitch_irc_manager: Arc<TwitchIRCManager>,
) -> impl Filter<Extract = (Arc<TwitchIRCManager>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || twitch_irc_manager.clone())
}

fn with_vrchat_manager(
    vrchat_manager: Arc<VRChatManager>,
) -> impl Filter<Extract = (Arc<VRChatManager>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || vrchat_manager.clone())
}