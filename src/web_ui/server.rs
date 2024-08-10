use std::env;

use warp::Filter;

use std::sync::Arc;

use tokio::sync::{broadcast, RwLock};

use warp::http::{HeaderMap, HeaderValue};

use crate::bot_status::BotStatus;

use crate::config::Config;

use crate::log_info;

use crate::LogLevel;

use crate::storage::StorageClient;

use crate::logging::Logger;

use crate::osc::VRChatOSC;

use crate::twitch::TwitchIRCClient;

use crate::web_ui::websocket_server;

use crate::web_ui::websocket_server::{DashboardState, update_dashboard_state};

use super::websocket::{handle_websocket, WebSocketMessage};

use super::api_routes::{api_routes, with_storage, with_logger};

use crate::CustomTwitchIRCClient;

use crate::discord::DiscordClient;



pub struct WebUI {

    config: Arc<RwLock<Config>>,

    storage: Arc<RwLock<StorageClient>>,

    logger: Arc<Logger>,

    pub dashboard_state: Arc<RwLock<websocket_server::DashboardState>>,

    discord_client: Option<Arc<DiscordClient>>,

}



impl WebUI {

    pub fn new(

        config: Arc<RwLock<Config>>,

        storage: Arc<RwLock<StorageClient>>,

        logger: Arc<Logger>,

        bot_status: Arc<RwLock<BotStatus>>,

        twitch_client: Arc<CustomTwitchIRCClient>,

        vrchat_osc: Option<Arc<VRChatOSC>>,

        discord_client: Option<Arc<DiscordClient>>,

    ) -> Self {

        let dashboard_state = Arc::new(RwLock::new(DashboardState::new(

            bot_status,

            config.clone(),

            Some(Arc::new(TwitchIRCClient::from(twitch_client.as_ref()))),

            vrchat_osc,

        )));



        WebUI {

            config,

            storage,

            logger,

            dashboard_state,

            discord_client,

        }

    }



    pub async fn run(&self) {

        let config = self.config.clone();

        let storage = self.storage.clone();

        let logger = self.logger.clone();

        let dashboard_state = self.dashboard_state.clone();



        // Get the path of the current executable

        let exe_path = env::current_exe().expect("Failed to get executable path");

        log_info!(self.logger, "Executable path: {:?}", exe_path);



        // Navigate to the project root (assuming the executable is in target/debug or target/release)

        let project_root = exe_path.parent().unwrap().parent().unwrap().parent().unwrap();

        log_info!(self.logger, "Project root: {:?}", project_root);



        // Construct the path to the build directory

        let build_path = project_root.join("web_ui").join("frontend").join("build");

        log_info!(self.logger, "Full path to build directory: {:?}", build_path);



        // Convert build_path to a String

        let build_path_str = build_path.to_str().unwrap().to_string();



        // Serve React app

        let static_files = {

            let logger = logger.clone();

            let build_path_str = build_path_str.clone();

            warp::path("ui").and(warp::fs::dir(build_path_str.clone()))

                .or(warp::path("static").and(warp::fs::dir(format!("{}/static", build_path_str))))

                .with(warp::log::custom(move |info| {

                    log_info!(logger, "Static file request: {} {} {}",

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

            .and(with_logger(logger.clone()))

            .map(|ws: warp::ws::Ws, dashboard_state, storage, logger| {

                ws.on_upgrade(move |socket| handle_websocket(socket, dashboard_state, storage, logger))

            });



        let api = api_routes(

            config.clone(),

            storage.clone(),

            logger.clone(),

            dashboard_state.clone()

        );



        let routes = {

            let logger = logger.clone();

            root_redirect

                .or(static_files)

                .or(catch_all)

                .or(ws_route)

                .or(api)

                .with(warp::log::custom(move |info| {

                    log_info!(logger, "Request: {} {} {}",

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



        log_info!(self.logger, "Starting web UI server on {}:{}", host, port);



        // Create a broadcast channel for WebSocket messages

        let (tx, _) = broadcast::channel::<WebSocketMessage>(100);



        // Start the periodic update task

        let update_task = tokio::spawn(update_dashboard_state(

            self.dashboard_state.clone(),

            self.storage.clone(),

            Arc::new(RwLock::new(self.discord_client.clone())),

            self.logger.clone(),

        ));



        // Run the server

        let addr: std::net::SocketAddr = format!("{}:{}", host, port).parse().expect("Invalid address");

        warp::serve(routes).run(addr).await;



        // Wait for the update task to finish (it should run indefinitely)

        let _ = update_task.await;

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