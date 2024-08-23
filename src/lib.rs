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

use bot_status::BotStatus;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use crate::twitch::irc::{TwitchIRCManager, TwitchBotClient, TwitchBroadcasterClient};
use crate::twitch::irc::message_handler::MessageHandler;
use crate::config::{Config, SocialLinks};
use crate::twitch::api::TwitchAPIClient;
use crate::vrchat::{VRChatClient, VRChatManager};
use crate::vrchat::World;
use crate::twitch::redeems::RedeemManager;
use crate::ai::AIClient;
use std::time::Duration;
use log::{debug, error, info, warn};
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;
use crate::discord::UserLinks;
use crate::osc::VRChatOSC;
use crate::osc::osc_config::OSCConfigurations;
use crate::storage::StorageClient;
use crate::twitch::eventsub::TwitchEventSubClient;
use crate::web_ui::websocket::{DashboardState};
use tokio::sync::mpsc;
use crate::obs::{OBSInstance, OBSManager, OBSStateUpdate};
use crate::twitch::TwitchManager;
use crate::web_ui::websocket::WebSocketMessage;

pub struct BotClients {
    pub twitch_manager: Arc<TwitchManager>,
    pub vrchat: Option<Arc<VRChatManager>>,
    pub obs: Option<Arc<OBSManager>>,
    pub discord: Option<Arc<discord::DiscordClient>>,
    pub ai_client: Option<Arc<AIClient>>,
    pub vrchat_osc: Option<Arc<VRChatOSC>>,
    pub storage: Arc<RwLock<StorageClient>>,
    pub bot_status: Arc<RwLock<BotStatus>>,
    pub dashboard_state: Arc<RwLock<DashboardState>>,
    pub websocket_tx: mpsc::UnboundedSender<WebSocketMessage>,
    pub websocket_rx: Option<mpsc::UnboundedReceiver<WebSocketMessage>>,
}

impl BotClients {
    async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        warn!("Initiating graceful shutdown...");

        // Notify users about shutdown
        self.notify_shutdown().await?;

        // Gracefully stop Twitch IRC clients
        warn!("Stopping Twitch IRC clients...");
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Disconnect from VRChat
        if let Some(vrchat_client) = &self.vrchat {
            warn!("Disconnecting from VRChat...");
            vrchat_client.disconnect().await?;
        }

        // Stop the EventSub client
        warn!("Stopping EventSub client...");
        // self.eventsub_client.lock().await.disconnect().await?;

        // Disconnect Discord client
        // if let Some(discord_client) = &self.discord {
        //     warn!("Disconnecting Discord client...");
        //     discord_client.disconnect().await?;
        // }

        // Stop the WebSocket server
        warn!("Stopping WebSocket server...");
        // You'll need to implement a method to stop the WebSocket server
        // This might involve closing all active connections and stopping the server

        // Stop the web UI server
        warn!("Stopping web UI server...");
        // You'll need to implement a method to stop the web UI server
        // This might involve shutting down the Warp server

        // Save final state
        warn!("Saving final redemption settings...");
        // self.redeem_manager.read().await.save_settings().await?;

        // Close storage connections
        warn!("Closing storage connections...");
        // self.storage.write().await.close().await?;

        info!("Shutdown complete.");
        self.bot_status.write().await.set_online(false);
        Ok(())
    }

    async fn notify_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let channel = self.get_twitch_channel()?;
        // self.twitch_bot_client.send_message(&channel, "MewBot is shutting down. Thank you for using our services!").await?;
        Ok(())
    }

    fn get_twitch_channel(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Implement this method to return the correct Twitch channel
        Ok("kittyn".to_string())
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

    let vrchat_osc = match VRChatOSC::new("127.0.0.1:9000") {
        Ok(osc) => {
            info!("VRChatOSC initialized successfully.");
            Some(Arc::new(osc))
        },
        Err(e) => {
            error!("Failed to initialize VRChatOSC: {}. OSC functionality will be disabled.", e);
            None
        }
    };

    let storage = Arc::new(RwLock::new(StorageClient::new("mewbot_data.db")?));
    let user_links = Arc::new(UserLinks::new());

    let discord = if config.read().await.is_discord_configured() {
        Some(Arc::new(discord::DiscordClient::new(
            config.clone(),
            storage.clone(),
            user_links.clone()
        ).await?))
    } else {
        None
    };

    let dashboard_state = Arc::new(RwLock::new(DashboardState::new(
        bot_status.clone(),
        config.clone(),
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
            dashboard_state.clone()
        ))
    });

    let config = Arc::new(config.read().await.clone());

    let twitch_manager = Arc::new(TwitchManager::new(
        config.clone(),
        storage.clone(),
        ai_client.clone(),
        vrchat_osc.clone(),
        user_links.clone(),
        dashboard_state.clone(),
        websocket_tx.clone(),
    ).await?);

    let clients = BotClients {
        twitch_manager,
        vrchat: vrchat_manager,
        obs: Some(obs_manager),
        discord,
        ai_client,
        vrchat_osc,
        storage,
        bot_status,
        dashboard_state,
        websocket_tx,
        websocket_rx: Some(websocket_rx),
    };

    clients.twitch_manager.redeem_manager.write().await.reset_coin_game().await?;
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
        clients.vrchat_osc.clone(),
        clients.discord.clone(),
        clients.dashboard_state.clone(),
        clients.obs.clone().expect("OBS manager should be initialized"),
        clients.vrchat.clone().expect("VRChat manager should be initialized"),
    ));

    let shutdown_signal = Arc::new(tokio::sync::Notify::new());

    let web_ui_handle = tokio::spawn({
        let web_ui = web_ui.clone();
        let shutdown_signal = shutdown_signal.clone();
        async move {
            web_ui.run(async move {
                shutdown_signal.notified().await;
            }).await
        }
    });

    info!("Initializing channel point redeems...");
    if let Err(e) = clients.twitch_manager.redeem_manager.write().await.initialize_redeems().await {
        error!("Failed to initialize channel point redeems: {}. Some redeems may not be available.", e);
    }

    info!("Setting up Twitch IRC message handling...");
    let channel = config.read().await.twitch_channel_to_join.clone()
        .ok_or("Twitch channel to join not set")?;

    if let Some(irc_client) = clients.twitch_manager.bot_client.get_client().await {
        if let Err(e) = clients.twitch_manager.redeem_manager.read().await.announce_redeems(&irc_client, &channel).await {
            error!("Failed to announce redeems: {}", e);
        }
    } else {
        error!("Failed to get IRC client for announcing redeems");
    }

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

    clients.twitch_manager.start_message_handler(message_handler.clone()).await?;

    let twitch_handler = tokio::spawn({
        let message_handler = message_handler.clone();
        let dashboard_state = clients.dashboard_state.clone();
        async move {
            let result = message_handler.handle_messages().await;
            if let Err(e) = &result {
                error!("Twitch handler error: {:?}", e);
                dashboard_state.write().await.update_twitch_status(false);
            }
            result
        }
    });
    handles.push(twitch_handler);

    info!("Twitch IRC handler started.");
    clients.dashboard_state.write().await.update_twitch_status(true);

    if let Some(discord_client) = &clients.discord {
        let discord_handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> = tokio::spawn({
            let discord_client = Arc::clone(discord_client);
            let dashboard_state = clients.dashboard_state.clone();
            async move {
                let result = discord_client.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>);

                if let Err(e) = &result {
                    error!("Discord client error: {:?}", e);
                    dashboard_state.write().await.update_discord_status(false);
                }
                result
            }
        });
        handles.push(discord_handle);
        clients.dashboard_state.write().await.update_discord_status(true);
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
                    dashboard_state.write().await.update_vrchat_status(false);
                }
                result
            }
        });
        handles.push(vrchat_handle);
        info!("VRChat websocket handler started.");
        clients.dashboard_state.write().await.update_vrchat_status(true);
    }

    // Start the WebSocket handler
    let ws_handle = tokio::spawn({
        let dashboard_state = clients.dashboard_state.clone();
        let storage = clients.storage.clone();
        let obs_manager = clients.obs.clone().expect("OBS manager should be initialized");
        let twitch_manager = clients.twitch_manager.clone();
        let vrchat_manager = clients.vrchat.clone().expect("VRChat manager should be initialized");
        let mut websocket_rx = clients.websocket_rx.take();
        async move {
            if let Some(mut rx) = websocket_rx {
                while let Some(msg) = rx.recv().await {
                    web_ui::websocket::handle_websocket(
                        msg,
                        dashboard_state.clone(),
                        storage.clone(),
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

    let ctrl_c_signal = shutdown_signal.clone();

    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        warn!("Received Ctrl+C, initiating shutdown.");
        ctrl_c_signal.notify_waiters();
    });

    let run_result = tokio::select! {
        _ = futures::future::join_all(handles) => {
            info!("All handlers have completed.");
            Ok(())
        }
        _ = shutdown_signal.notified() => {
            warn!("Shutdown signal received, stopping all tasks.");
            clients.twitch_manager.shutdown().await
        }
    };

    shutdown_signal.notify_waiters();

    if let Err(e) = web_ui_handle.await? {
        error!("Web UI error during shutdown: {:?}", e);
    }

    info!("Bot has shut down.");
    run_result
}