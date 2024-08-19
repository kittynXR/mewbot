pub mod config;
pub mod twitch;
pub mod vrchat;
pub mod discord;
pub mod ai;
pub mod osc;
pub mod storage;
pub mod web_ui;
mod bot_status;

use bot_status::BotStatus;
use crate::twitch::role_cache::RoleCache;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use crate::twitch::irc::{TwitchIRCManager, TwitchBotClient, TwitchBroadcasterClient};
use crate::twitch::irc::message_handler::MessageHandler;
use crate::config::Config;
use crate::twitch::api::TwitchAPIClient;
use crate::vrchat::VRChatClient;
use crate::vrchat::World;
use crate::twitch::redeems::RedeemManager;
use crate::ai::AIClient;
use std::time::Duration;
use log::{debug, error, info, warn};
use tokio::task::JoinHandle;
use crate::discord::UserLinks;
use crate::osc::VRChatOSC;
use crate::osc::osc_config::OSCConfigurations;
use crate::storage::StorageClient;
use crate::twitch::eventsub::TwitchEventSubClient;
use crate::web_ui::websocket::{DashboardState, WorldState};
use tokio::sync::mpsc;
use crate::web_ui::websocket::WebSocketMessage;

pub struct BotClients {
    pub twitch_irc_manager: Arc<TwitchIRCManager>,
    pub twitch_bot_client: Arc<TwitchBotClient>,
    pub twitch_broadcaster_client: Option<Arc<TwitchBroadcasterClient>>,
    pub twitch_api: Option<Arc<TwitchAPIClient>>,
    pub vrchat: Option<Arc<VRChatClient>>,
    pub discord: Option<Arc<discord::DiscordClient>>,
    pub redeem_manager: Arc<RwLock<RedeemManager>>,
    pub ai_client: Option<Arc<AIClient>>,
    pub eventsub_client: Arc<Mutex<TwitchEventSubClient>>,
    pub vrchat_osc: Option<Arc<VRChatOSC>>,
    pub storage: Arc<RwLock<storage::StorageClient>>,
    pub role_cache: Arc<RwLock<RoleCache>>,
    pub user_links: Arc<UserLinks>,
    pub bot_status: Arc<RwLock<BotStatus>>,
    pub dashboard_state: Arc<RwLock<DashboardState>>,
    pub websocket_tx: mpsc::Sender<WebSocketMessage>,
    pub websocket_rx: Option<mpsc::Receiver<WebSocketMessage>>,
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
        self.redeem_manager.read().await.save_settings().await?;

        // Close storage connections
        warn!("Closing storage connections...");
        // self.storage.write().await.close().await?;

        info!("Shutdown complete.");
        self.bot_status.write().await.set_online(false);
        Ok(())
    }

    async fn notify_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let channel = self.get_twitch_channel()?;
        self.twitch_bot_client.send_message(&channel, "MewBot is shutting down. Thank you for using our services!").await?;
        Ok(())
    }

    fn get_twitch_channel(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Implement this method to return the correct Twitch channel
        Ok("kittyn".to_string())
    }
}

pub async fn init(config: Arc<RwLock<Config>>) -> Result<BotClients, Box<dyn std::error::Error + Send + Sync>> {
    let bot_status = BotStatus::new();

    let (websocket_tx, websocket_rx) = mpsc::channel::<WebSocketMessage>(100);

    let discord_link = config.read().await.discord_link.clone().unwrap_or_default();
    let twitch_irc_manager = Arc::new(TwitchIRCManager::new(websocket_tx.clone(), discord_link));

    let twitch_api = if config.read().await.is_twitch_api_configured() {
        let api_client = TwitchAPIClient::new(config.clone()).await?;
        api_client.authenticate().await?;
        Some(Arc::new(api_client))
    } else {
        None
    };

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

    let redeem_manager = Arc::new(RwLock::new(RedeemManager::new(
        ai_client.clone(),
        vrchat_osc.clone(),
        twitch_api.clone().ok_or("Twitch API client not initialized")?,
    )));

    let mut twitch_bot_client = None;
    let mut twitch_broadcaster_client = None;

    if config.read().await.is_twitch_irc_configured() {
        info!("Initializing Twitch IRC clients...");

        let config_read = config.read().await;
        let bot_username = config_read.twitch_bot_username.clone().ok_or("Twitch IRC bot username not set")?;
        let bot_oauth_token = config_read.twitch_bot_oauth_token.clone().ok_or("Bot OAuth token not set")?;
        let broadcaster_username = config_read.twitch_channel_to_join.clone().ok_or("Twitch channel to join not set")?;
        let broadcaster_oauth_token = config_read.twitch_broadcaster_oauth_token.clone().ok_or("Broadcaster OAuth token not set")?;
        let channel = broadcaster_username.clone();

        twitch_irc_manager.add_client(bot_username.clone(), bot_oauth_token.clone(), vec![channel.clone()]).await?;
        twitch_bot_client = Some(Arc::new(TwitchBotClient::new(bot_username, twitch_irc_manager.clone())));

        if let Some(broadcaster_oauth_token) = config_read.twitch_access_token.clone() {
            twitch_irc_manager.add_client(broadcaster_username.clone(), broadcaster_oauth_token, vec![channel.clone()]).await?;
            twitch_broadcaster_client = Some(Arc::new(TwitchBroadcasterClient::new(broadcaster_username, twitch_irc_manager.clone())));
        }

        info!("Twitch IRC clients initialized successfully.");
    } else {
        warn!("Twitch IRC is not configured. Skipping initialization.");
    }

    let osc_configs = Arc::new(RwLock::new(OSCConfigurations::load("osc_config.json").unwrap_or_default()));

    let eventsub_client = if let (Some(bot_client), Some(api_client)) = (&twitch_bot_client, &twitch_api) {
        let channel = config.read().await.twitch_channel_to_join.clone().ok_or("Twitch channel to join not set")?;
        let bot_irc_client = bot_client.get_client().await.ok_or("Failed to get IRC client")?;
        Arc::new(Mutex::new(TwitchEventSubClient::new(
            config.clone(),
            Arc::clone(api_client),
            bot_irc_client,
            channel,
            redeem_manager.clone(),
            ai_client.clone(),
            vrchat_osc.clone(),
            osc_configs.clone(),
        )))
    } else {
        return Err("Both Twitch IRC and API clients must be initialized for EventSub".into());
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

    let role_cache = Arc::new(RwLock::new(RoleCache::new()));

    let dashboard_state = Arc::new(RwLock::new(DashboardState::new(
        bot_status.clone(),
        config.clone(),
        Some(twitch_irc_manager.clone()),
        vrchat_osc.clone(),
    )));

    let clients = BotClients {
        twitch_irc_manager: twitch_irc_manager.clone(),
        twitch_bot_client: twitch_bot_client.ok_or("Twitch bot client not initialized")?,
        twitch_broadcaster_client,
        twitch_api,
        vrchat,
        discord,
        redeem_manager,
        ai_client,
        eventsub_client,
        vrchat_osc,
        storage,
        role_cache,
        user_links,
        bot_status,
        dashboard_state,
        websocket_tx,
        websocket_rx: Some(websocket_rx),
    };

    clients.redeem_manager.write().await.reset_coin_game().await?;
    let eventsub_client_clone = clients.eventsub_client.clone();
    tokio::spawn(async move {
        let client = eventsub_client_clone.lock().await;
        if let Err(e) = client.check_current_stream_status().await {
            eprintln!("Failed to check current stream status: {:?}", e);
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
        clients.twitch_irc_manager.clone(),
        clients.vrchat_osc.clone(),
        clients.discord.clone(),
        clients.dashboard_state.clone(), // Pass the shared DashboardState
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

    let world_info = Arc::new(Mutex::new(None::<World>));
    let world_state = Arc::new(RwLock::new(WorldState::new()));

    info!("Initializing channel point redeems...");
    if let Err(e) = clients.redeem_manager.write().await.initialize_redeems().await {
        error!("Failed to initialize channel point redeems: {}. Some redeems may not be available.", e);
    }

    if let Some(api_client) = &clients.twitch_api {
        info!("Setting up Twitch IRC message handling...");

        let world_info_clone = Arc::clone(&world_info);
        let api_client = Arc::clone(api_client);

        let channel = config.read().await.twitch_channel_to_join.clone()
            .ok_or("Twitch channel to join not set")?;

        if let Some(irc_client) = clients.twitch_bot_client.get_client().await {
            if let Err(e) = clients.redeem_manager.read().await.announce_redeems(&irc_client, &channel).await {
                error!("Failed to announce redeems: {}", e);
            }
        } else {
            error!("Failed to get IRC client for announcing redeems");
        }

        let message_handler = Arc::new(MessageHandler::new(
            clients.twitch_bot_client.clone(),
            config.clone(),
            api_client.clone(),
            clients.redeem_manager.clone(),
            clients.storage.clone(),
            clients.role_cache.clone(),
            clients.user_links.clone(),
            clients.websocket_tx.clone(),
            world_info.clone(),
            clients.vrchat.clone().expect("VRChatClient should be initialized"),
            clients.ai_client.clone(),  // Add this line to pass the AI client
        ));

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
    }

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

    let eventsub_client = clients.eventsub_client.clone();
    let eventsub_handle = tokio::spawn({
        async move {
            info!("Starting EventSub client");
            if let Err(e) = eventsub_client.lock().await.connect_and_listen().await {
                error!("EventSub client error: {:?}", e);
            }
            Ok(()) as Result<(), Box<dyn std::error::Error + Send + Sync>>
        }
    });
    handles.push(eventsub_handle);
    info!("EventSub client started.");

    let eventsub_client_clone = clients.eventsub_client.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
            if let Err(e) = eventsub_client_clone.lock().await.refresh_token_periodically().await {
                error!("Failed to refresh token: {:?}", e);
            }
        }
    });

    // Pass dashboard_state to the VRChat handler
    if let Some(vrchat_client) = &clients.vrchat {
        let current_user_id = vrchat_client.get_current_user_id().await?;
        let auth_cookie = vrchat_client.get_auth_cookie().await;
        let vrchat_handle = tokio::spawn({
            let dashboard_state = clients.dashboard_state.clone();
            let vrchat_client = Arc::clone(vrchat_client);
            async move {
                let result = crate::vrchat::websocket::handler(
                    auth_cookie,
                    world_state.clone(),
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

    clients.twitch_irc_manager.flush_initial_messages().await;

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
            clients.shutdown().await
        }
    };

    shutdown_signal.notify_waiters();

    if let Err(e) = web_ui_handle.await? {
        error!("Web UI error during shutdown: {:?}", e);
    }

    info!("Bot has shut down.");
    run_result
}