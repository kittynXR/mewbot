pub mod config;
pub mod twitch;
pub mod vrchat;
pub mod discord;
pub mod ai;
pub mod osc;
pub mod storage;
pub mod web_ui;
mod bot_status;
pub mod obs;
pub mod stream_status;

pub mod stream_state;

use bot_status::BotStatus;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use crate::twitch::irc::message_handler::MessageHandler;
use crate::config::{Config};
use crate::vrchat::{VRChatClient, VRChatManager};
use crate::vrchat::World;
use crate::ai::AIClient;
use std::time::Duration;
use log::{error, info, warn};
use tokio::task::JoinHandle;
use crate::discord::UserLinks;
use crate::osc::{OSCManager};
use crate::storage::StorageClient;
use crate::web_ui::websocket::{DashboardState};
use tokio::sync::mpsc;
use crate::obs::{OBSInstance, OBSManager};
use crate::stream_state::StreamStateMachine;
use crate::twitch::TwitchManager;
use crate::web_ui::websocket::WebSocketMessage;

pub struct BotClients {
    pub twitch_manager: Arc<TwitchManager>,
    pub vrchat: Option<Arc<VRChatManager>>,
    pub obs: Option<Arc<OBSManager>>,
    pub discord: Option<Arc<discord::DiscordClient>>,
    pub ai_client: Option<Arc<AIClient>>,
    pub osc_manager: Arc<OSCManager>,
    pub storage: Arc<RwLock<StorageClient>>,
    pub bot_status: Arc<RwLock<BotStatus>>,
    pub dashboard_state: Arc<RwLock<DashboardState>>,
    pub websocket_tx: mpsc::UnboundedSender<WebSocketMessage>,
    pub websocket_rx: Option<mpsc::UnboundedReceiver<WebSocketMessage>>,
}

impl BotClients {
    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        warn!("Initiating graceful shutdown...");

        let shutdown_timeout = Duration::from_secs(15);

        let (_twitch_result, _vrchat_result, _obs_result, _discord_result) = tokio::join!(
        tokio::time::timeout(shutdown_timeout, self.twitch_manager.shutdown()),
        tokio::time::timeout(shutdown_timeout, async {
            if let Some(vrchat) = &self.vrchat {
                vrchat.shutdown().await
            } else {
                Ok(())
            }
        }),
        tokio::time::timeout(shutdown_timeout, async {
            if let Some(obs) = &self.obs {
                obs.shutdown().await
            } else {
                Ok(())
            }
        }),
        tokio::time::timeout(shutdown_timeout, async {
            if let Some(discord) = &self.discord {
                discord.shutdown().await
            } else {
                Ok(())
            }
        })
    );

        // Handle results and log any errors

        info!("Closing storage connections...");
        match tokio::time::timeout(shutdown_timeout, self.storage.write().await.close()).await {
            Ok(Ok(_)) => info!("Storage connections closed successfully"),
            Ok(Err(e)) => error!("Error closing storage connections: {:?}", e),
            Err(_) => error!("Timed out while closing storage connections"),
        }

        info!("All modules shut down.");
        self.bot_status.write().await.set_online(false);
        Ok(())
    }
}

pub async fn init(config: Arc<RwLock<Config>>) -> Result<BotClients, Box<dyn std::error::Error + Send + Sync>> {
    let bot_status = BotStatus::new();

    let (websocket_tx, websocket_rx) = mpsc::unbounded_channel::<WebSocketMessage>();

    let config_read = config.read().await;
    let ai_client = if let Some(openai_secret) = &config_read.openai_secret {
        Some(Arc::new(AIClient::new(
            Some(openai_secret.clone()),
            config_read.anthropic_secret.clone(),
        )))
    } else {
        None
    };
    drop(config_read);

    let vrchat = if config.read().await.is_vrchat_configured() {
        match VRChatClient::new(config.clone(), websocket_tx.clone()).await {
            Ok(vrchat_client) => {
                info!("VRChat client initialized successfully.");
                Some(Arc::new(vrchat_client))
            }
            Err(e) => {
                error!("Failed to initialize VRChat client: {}. VRChat functionality will be disabled.", e);
                None
            }
        }
    } else {
        None
    };

    let osc_manager = match OSCManager::new("127.0.0.1:9000").await {
        Ok(manager) => {
            info!("OSCManager initialized successfully.");
            Arc::new(manager)
        }
        Err(e) => {
            error!("Failed to initialize OSCManager: {}. OSC functionality will be disabled.", e);
            return Err(Box::new(e));
        }
    };

    let storage = Arc::new(RwLock::new(StorageClient::new("mewbot_data.db")?));
    let user_links = Arc::new(UserLinks::new());

    let discord = if config.read().await.is_discord_configured() {
        Some(Arc::new(discord::DiscordClient::new(
            config.clone(),
            // storage.clone(),
            user_links.clone()
        ).await?))
    } else {
        None
    };

    let dashboard_state = Arc::new(RwLock::new(DashboardState::new(
        bot_status.clone(),
    )));

    let obs_manager = Arc::new(OBSManager::new(websocket_tx.clone()));

    // Initialize OBS instances from config
    let obs_config = config.read().await.obs_manager.clone();
    if let Err(e) = obs_manager.add_instance("Instance1".to_string(), OBSInstance {
        name: "Instance1".to_string(),
        address: obs_config.instance1.ip.clone(),
        port: obs_config.instance1.port,
        auth_required: obs_config.instance1.auth_required,
        password: obs_config.instance1.password.clone(),
        use_ssl: obs_config.instance1.use_ssl,
    }).await {
        warn!("Failed to add OBS Instance1: {}. Continuing without this instance.", e);
    }

    if obs_config.is_dual_pc_setup {
        if let Some(instance2) = obs_config.instance2 {
            if let Err(e) = obs_manager.add_instance("Instance2".to_string(), OBSInstance {
                name: "Instance2".to_string(),
                address: instance2.ip.clone(),
                port: instance2.port,
                auth_required: instance2.auth_required,
                password: instance2.password.clone(),
                use_ssl: instance2.use_ssl,
            }).await {
                warn!("Failed to add OBS Instance2: {}. Continuing without this instance.", e);
            }
        }
    }

    let vrchat_manager = vrchat.as_ref().map(|vrchat_client| {
        Arc::new(VRChatManager::new(
            Arc::clone(vrchat_client),
            dashboard_state.clone(),
            Some(osc_manager.clone()),  // Wrap in Some()
        ))
    });

    let config = Arc::new(config.read().await.clone());
    let stream_state_machine = StreamStateMachine::new();

    let mut twitch_manager = TwitchManager::new(
        config,
        storage.clone(),
        ai_client.clone(),
        osc_manager.clone(),
        user_links,
        dashboard_state.clone(),
        websocket_tx.clone(),
        stream_state_machine.clone(),
    ).await?;

    twitch_manager.initialize().await?;

    let clients = BotClients {
        twitch_manager: twitch_manager.clone().into(),
        vrchat: vrchat_manager,
        obs: Some(obs_manager),
        discord,
        ai_client,
        osc_manager,
        storage,
        bot_status,
        dashboard_state,
        websocket_tx,
        websocket_rx: Some(websocket_rx),
    };

    // twitch_manager.redeem_manager.read().await.initialize_redeems().await?;

    // clients.twitch_manager.redeem_manager.write().await.reset_coin_game().await?;
    let eventsub_client_clone = clients.twitch_manager.eventsub_client.clone();
    tokio::spawn(async move {
        let client = eventsub_client_clone.lock().await;
        if let Some(client) = client.as_ref() {
            if let Err(e) = client.check_current_stream_status().await {
                eprintln!("Failed to check current stream status: {:?}", e);
            }
        } else {
            eprintln!("EventSub client is not initialized");
        }
    });

    clients.bot_status.write().await.set_online(true);

    Ok(clients)
}

pub async fn run(mut clients: BotClients, config: Arc<RwLock<Config>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut handles: Vec<JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>> = vec![];

    let web_ui = Arc::new(web_ui::WebUI::new(
        config.clone(),
        clients.storage.clone(),
        clients.bot_status.clone(),
        clients.twitch_manager.irc_manager.clone(),
        clients.dashboard_state.clone(),
        clients.obs.clone().expect("OBS manager should be initialized"),
        clients.vrchat.clone().expect("VRChat manager should be initialized"),
    ));

    let shutdown_signal = Arc::new(tokio::sync::Notify::new());

    let _web_ui_handle = tokio::spawn({
        let web_ui = web_ui.clone();
        let shutdown_signal = shutdown_signal.clone();
        async move {
            web_ui.run(async move {
                shutdown_signal.notified().await;
            }).await
        }
    });

    info!("Initializing channel point redeems...");
    if let Some(redeem_manager) = clients.twitch_manager.redeem_manager.write().await.as_mut() {
        redeem_manager.initialize_redeems().await?;
    }

    info!("Setting up Twitch IRC message handling...");
    let _channel = config.read().await.twitch_channel_to_join.clone()
        .ok_or("Twitch channel to join not set")?;

    // if let Some(irc_client) = clients.twitch_manager.bot_client.get_client().await {
    //     if let Err(e) = clients.twitch_manager.redeem_manager.read().await.announce_redeems(&irc_client, &channel).await {
    //         error!("Failed to announce redeems: {}", e);
    //     }
    // } else {
    //     error!("Failed to get IRC client for announcing redeems");
    // }

    let world_info = Arc::new(Mutex::new(None::<World>));
    let message_handler = Arc::new(MessageHandler::new(
        config.clone(),
        clients.twitch_manager.clone(),
        clients.storage.clone(),
        clients.websocket_tx.clone(),
        world_info,
        clients.vrchat.clone().expect("VRChatClient should be initialized"),
        clients.ai_client.clone(),
    ));

    // Start only one message handler
    let twitch_handler = tokio::spawn({
        let message_handler = message_handler.clone();
        let dashboard_state = clients.dashboard_state.clone();
        async move {
            let result = message_handler.handle_messages().await;
            if let Err(e) = &result {
                error!("Twitch handler error: {:?}", e);
                dashboard_state.write().await.update_twitch_status(false).await;
            }
            result
        }
    });
    handles.push(twitch_handler);

    info!("Twitch IRC handler started.");
    clients.dashboard_state.write().await.update_twitch_status(true).await;

    if let Some(discord_client) = &clients.discord {
        let discord_handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> = tokio::spawn({
            let discord_client = Arc::clone(discord_client);
            let dashboard_state = clients.dashboard_state.clone();
            async move {
                let result = discord_client.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>);

                if let Err(e) = &result {
                    error!("Discord client error: {:?}", e);
                    dashboard_state.write().await.update_discord_status(false).await;
                }
                result
            }
        });
        handles.push(discord_handle);
        clients.dashboard_state.write().await.update_discord_status(true).await;
    }

    let eventsub_client = clients.twitch_manager.eventsub_client.clone();
    let eventsub_handle = tokio::spawn({
        let eventsub_client = eventsub_client.clone();
        async move {
            info!("Starting EventSub client");
            let client = eventsub_client.lock().await;
            if let Some(client) = client.as_ref() {
                if let Err(e) = client.connect_and_listen().await {
                    error!("EventSub client error: {:?}", e);
                }
            } else {
                error!("EventSub client is not initialized");
            }
            Ok(()) as Result<(), Box<dyn std::error::Error + Send + Sync>>
        }
    });
    handles.push(eventsub_handle);
    info!("EventSub client started.");

    let eventsub_client_clone = clients.twitch_manager.eventsub_client.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            let client = eventsub_client_clone.lock().await;
            if let Some(client) = client.as_ref() {
                if let Err(e) = client.refresh_token_periodically().await {
                    error!("Failed to refresh token: {:?}", e);
                }
            } else {
                error!("EventSub client is not initialized");
            }
        }
    });

    if let Some(vrchat_client) = &clients.vrchat {
        let current_user_id = vrchat_client.get_current_user_id().await?;
        let auth_cookie = vrchat_client.get_auth_cookie().await;
        let vrchat_handle = tokio::spawn({
            let dashboard_state = clients.dashboard_state.clone();
            let vrchat_client = Arc::clone(vrchat_client);
            async move {
                let result = crate::vrchat::websocket::handler(
                    auth_cookie,
                    current_user_id,
                    vrchat_client,
                    dashboard_state.clone()
                ).await;
                if let Err(e) = &result {
                    error!("VRChat websocket handler error: {:?}", e);
                    dashboard_state.write().await.update_vrchat_status(false).await;
                }
                result
            }
        });
        handles.push(vrchat_handle);
        info!("VRChat websocket handler started.");
        clients.dashboard_state.write().await.update_vrchat_status(true).await;
    }

    // Start the WebSocket handler
    let ws_handle = tokio::spawn({
        let obs_manager = clients.obs.clone().expect("OBS manager should be initialized");
        let twitch_manager = clients.twitch_manager.clone();
        let vrchat_manager = clients.vrchat.clone().expect("VRChat manager should be initialized");
        let websocket_rx = clients.websocket_rx.take();
        async move {
            if let Some(mut rx) = websocket_rx {
                while let Some(msg) = rx.recv().await {
                    web_ui::websocket::handle_websocket(
                        msg,
                        obs_manager.clone(),
                        twitch_manager.irc_manager.clone(),
                        vrchat_manager.clone(),
                    ).await;
                }
            }
            Ok(())
        }
    });
    handles.push(ws_handle);

    info!("Bot is now running. Press Ctrl+C to exit.");

    let _ctrl_c_signal = shutdown_signal.clone();

    let shutdown_signal = Arc::new(tokio::sync::Notify::new());
    let shutdown_signal_clone = shutdown_signal.clone();

    let (web_ui_shutdown_tx, web_ui_shutdown_rx) = tokio::sync::oneshot::channel();

    let web_ui_handle = tokio::spawn({
        let web_ui = web_ui.clone();
        async move {
            web_ui.run(async move {
                web_ui_shutdown_rx.await.ok();
            }).await
        }
    });

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        warn!("Received Ctrl+C, initiating shutdown.");
        shutdown_signal_clone.notify_waiters();
    });

    let run_result = tokio::select! {
        results = futures::future::join_all(handles) => {
            info!("All handlers have completed.");
            for (index, result) in results.into_iter().enumerate() {
                match result {
                    Ok(Ok(())) => info!("Task {} completed successfully.", index),
                    Ok(Err(e)) => warn!("Task {} ended with error: {:?}", index, e),
                    Err(e) => warn!("Task {} was cancelled or panicked: {:?}", index, e),
                }
            }
            Ok(())
        }
        _ = shutdown_signal.notified() => {
            warn!("Shutdown signal received, stopping all tasks.");

            // Signal the Web UI to shut down
            let _ = web_ui_shutdown_tx.send(());

            // Wait for the Web UI to shut down (with a timeout)
            match tokio::time::timeout(std::time::Duration::from_secs(10), web_ui_handle).await {
                Ok(Ok(_)) => info!("Web UI shut down successfully."),
                Ok(Err(e)) => warn!("Web UI shut down with error: {:?}", e),
                Err(_) => warn!("Timed out waiting for Web UI to shut down."),
            }

            clients.shutdown().await
        }
    };

    info!("Bot has shut down.");
    run_result
}