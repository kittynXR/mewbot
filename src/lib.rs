pub mod config;
pub mod twitch;
pub mod vrchat;
pub mod discord;
pub mod logging;
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
use tokio::task::JoinHandle;
use crate::discord::UserLinks;
use crate::logging::{LogLevel, Logger};
use crate::osc::VRChatOSC;
use crate::osc::osc_config::OSCConfigurations;
use crate::storage::StorageClient;
use crate::twitch::eventsub::TwitchEventSubClient;
use crate::web_ui::websocket::DashboardState;
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
    pub logger: Arc<Logger>,
    pub bot_status: Arc<RwLock<BotStatus>>,
    pub dashboard_state: Arc<RwLock<DashboardState>>,
    pub websocket_tx: mpsc::Sender<WebSocketMessage>,
    pub websocket_rx: Option<mpsc::Receiver<WebSocketMessage>>,
}

impl BotClients {
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Initiating graceful shutdown...");

        // Notify users about shutdown
        self.notify_shutdown().await?;

        // Gracefully stop Twitch IRC clients
        println!("Stopping Twitch IRC clients...");
        let channel = self.get_twitch_channel()?;
        self.twitch_bot_client.send_message(&channel, "MewBot is shutting down. Goodbye!").await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Disconnect from VRChat
        if let Some(vrchat_client) = &self.vrchat {
            println!("Disconnecting from VRChat...");
            vrchat_client.disconnect().await?;
        }

        // Save final state
        println!("Saving final redemption settings...");
        self.redeem_manager.read().await.save_settings().await?;

        println!("Shutdown complete.");
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
    let logger = Arc::new(Logger::new(config.clone()));
    let bot_status = BotStatus::new(logger.clone());

    let (websocket_tx, websocket_rx) = mpsc::channel::<WebSocketMessage>(100);

    let twitch_irc_manager = Arc::new(TwitchIRCManager::new(websocket_tx.clone(), logger.clone()));

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
                println!("VRChat client initialized successfully.");
                Some(Arc::new(vrchat_client))
            }
            Err(e) => {
                eprintln!("Failed to initialize VRChat client: {}. VRChat functionality will be disabled.", e);
                None
            }
        }
    } else {
        None
    };

    let vrchat_osc = match VRChatOSC::new("127.0.0.1:9000") {
        Ok(osc) => {
            println!("VRChatOSC initialized successfully.");
            Some(Arc::new(osc))
        },
        Err(e) => {
            eprintln!("Failed to initialize VRChatOSC: {}. OSC functionality will be disabled.", e);
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
        println!("Initializing Twitch IRC clients...");

        let config_read = config.read().await;
        let bot_username = config_read.twitch_bot_username.clone().ok_or("Twitch IRC bot username not set")?;
        let bot_oauth_token = config_read.twitch_bot_oauth_token.clone().ok_or("Bot OAuth token not set")?;
        let broadcaster_username = config_read.twitch_channel_to_join.clone().ok_or("Twitch channel to join not set")?;
        let broadcaster_oauth_token = config_read.twitch_broadcaster_oauth_token.clone().ok_or("Broadcaster OAuth token not set")?;
        let channel = broadcaster_username.clone();

        println!("Twitch IRC bot username: {}", bot_username);
        println!("Twitch IRC OAuth token (first 10 chars): {}...", &bot_oauth_token[..10]);
        println!("Twitch channel to join: {}", channel);
        println!("Bot OAuth token (first 10 chars): {}...", &broadcaster_oauth_token[..10]);

        twitch_irc_manager.add_client(bot_username.clone(), bot_oauth_token.clone(), vec![channel.clone()]).await?;
        twitch_bot_client = Some(Arc::new(TwitchBotClient::new(bot_username, twitch_irc_manager.clone())));

        if let Some(broadcaster_oauth_token) = config_read.twitch_access_token.clone() {
            twitch_irc_manager.add_client(broadcaster_username.clone(), broadcaster_oauth_token, vec![channel.clone()]).await?;
            twitch_broadcaster_client = Some(Arc::new(TwitchBroadcasterClient::new(broadcaster_username, twitch_irc_manager.clone())));
        }

        println!("Twitch IRC clients initialized successfully.");
    } else {
        println!("Twitch IRC is not configured. Skipping initialization.");
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
            logger.clone(),
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

    let web_ui = Arc::new(web_ui::WebUI::new(
        config.clone(),
        storage.clone(),
        logger.clone(),
        bot_status.clone(),
        twitch_irc_manager.clone(),
        vrchat_osc.clone(),
        discord.clone(),
    ));

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
        logger: logger.clone(),
        bot_status,
        dashboard_state,
        websocket_tx,
        websocket_rx:  Some(websocket_rx), // Add this line
    };

    // Reset coin game and update redeems based on stream status
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
        clients.logger.clone(),
        clients.bot_status.clone(),
        clients.twitch_irc_manager.clone(),
        clients.vrchat_osc.clone(),
        clients.discord.clone(),
    ));

    let web_ui_clone = web_ui.clone();

    let web_ui_handle = tokio::spawn(async move {
        web_ui_clone.run().await;
    });

    let world_info = Arc::new(Mutex::new(None::<World>));

    // Initialize redeems
    println!("Initializing channel point redeems...");
    if let Err(e) = clients.redeem_manager.write().await.initialize_redeems().await {
        eprintln!("Failed to initialize channel point redeems: {}. Some redeems may not be available.", e);
    }

    if let Some(api_client) = &clients.twitch_api {
        println!("Setting up Twitch IRC message handling...");

        let world_info_clone = Arc::clone(&world_info);
        let api_client = Arc::clone(api_client);

        let channel = config.read().await.twitch_channel_to_join.clone()
            .ok_or("Twitch channel to join not set")?;

        // Announce redeems after initialization
        if let Some(irc_client) = clients.twitch_bot_client.get_client().await {
            if let Err(e) = clients.redeem_manager.read().await.announce_redeems(&irc_client, &channel).await {
                eprintln!("Failed to announce redeems: {}", e);
            }
        } else {
            eprintln!("Failed to get IRC client for announcing redeems");
        }

        let message_handler = Arc::new(MessageHandler::new(
            clients.twitch_bot_client.clone(),
            config.clone(),
            api_client,
            clients.redeem_manager.clone(),
            clients.storage.clone(),
            clients.role_cache.clone(),
            clients.user_links.clone(),
            clients.logger.clone(),
            clients.websocket_tx.clone(),
            world_info.clone(),
            clients.vrchat.clone().expect("VRChatClient should be initialized") // Add this line
        ));

        let twitch_handler = tokio::spawn({
            let message_handler = message_handler.clone();
            let dashboard_state = clients.dashboard_state.clone();
            let logger = clients.logger.clone();
            async move {
                let result = message_handler.handle_messages().await;
                if let Err(e) = &result {
                    log_error!(logger, "Twitch handler error: {:?}", e);
                    dashboard_state.write().await.update_twitch_status(false);
                }
                result
            }
        });
        handles.push(twitch_handler);
        println!("Twitch IRC handler started.");
        clients.dashboard_state.write().await.update_twitch_status(true);
    }

    let websocket_handle = tokio::spawn({
        let dashboard_state = clients.dashboard_state.clone();
        let logger = clients.logger.clone();
        let mut websocket_rx = clients.websocket_rx.take().expect("WebSocket receiver missing");
        async move {
            while let Some(message) = websocket_rx.recv().await {
                if let Err(e) = dashboard_state.write().await.broadcast_message(message).await {
                    log_error!(logger, "Failed to broadcast WebSocket message: {:?}", e);
                }
            }
            Ok(()) as Result<(), Box<dyn std::error::Error + Send + Sync>>
        }
    });
    handles.push(websocket_handle);

    if let Some(discord_client) = clients.discord {
        let discord_handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> = tokio::spawn({
            let dashboard_state = clients.dashboard_state.clone();
            let logger_clone = clients.logger.clone();
            async move {
                let result = match Arc::try_unwrap(discord_client) {
                    Ok(client) => client.start().await.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                    Err(_) => Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to unwrap Arc")) as Box<dyn std::error::Error + Send + Sync>),
                };

                if let Err(e) = &result {
                    log_error!(logger_clone, "Discord client error: {:?}", e);
                    dashboard_state.write().await.update_discord_status(false);
                }
                result
            }
        });
        handles.push(discord_handle);
        clients.dashboard_state.write().await.update_discord_status(true);
    }

    // EventSub client handling
    let eventsub_client = clients.eventsub_client.clone();
    let eventsub_handle = tokio::spawn({
        let logger_clone = clients.logger.clone();
        async move {
            log_info!(logger_clone, "Starting EventSub client");
            if let Err(e) = eventsub_client.lock().await.connect_and_listen().await {
                log_error!(logger_clone, "EventSub client error: {:?}", e);
            }
            Ok(()) as Result<(), Box<dyn std::error::Error + Send + Sync>>
        }
    });
    handles.push(eventsub_handle);
    println!("EventSub client started.");

    // Start the token refresh task
    let eventsub_client_clone = clients.eventsub_client.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await; // 1 hour
            if let Err(e) = eventsub_client_clone.lock().await.refresh_token_periodically().await {
                println!("Failed to refresh token: {:?}", e);
            }
        }
    });

    if let Some(vrchat_client) = &clients.vrchat {
        let current_user_id = vrchat_client.get_current_user_id().await?;
        let auth_cookie = vrchat_client.get_auth_cookie().await;
        let vrchat_handle = tokio::spawn({
            let logger_clone = clients.logger.clone();
            let dashboard_state = clients.dashboard_state.clone();
            let websocket_tx = clients.websocket_tx.clone();
            let vrchat_client = Arc::clone(vrchat_client);
            async move {
                let result = crate::vrchat::websocket::handler(
                    auth_cookie,
                    world_info.clone(),
                    current_user_id,
                    vrchat_client,
                    websocket_tx
                ).await;
                if let Err(e) = &result {
                    log_error!(logger_clone, "VRChat websocket handler error: {:?}", e);
                    dashboard_state.write().await.update_vrchat_status(false);
                }
                result
            }
        });
        handles.push(vrchat_handle);
        println!("VRChat websocket handler started.");
        clients.dashboard_state.write().await.update_vrchat_status(true);
    }

    println!("Bot is now running. Press Ctrl+C to exit.");

    tokio::select! {
        _ = futures::future::join_all(handles) => {
            println!("All handlers have completed.");
        }
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl+C, shutting down.");
        }
    }

    web_ui_handle.await?;
    println!("Bot has shut down.");
    Ok(())
}

async fn handle_twitch_messages(
    twitch_bot_client: Arc<TwitchBotClient>,
    config: Arc<RwLock<Config>>,
    api_client: Arc<TwitchAPIClient>,
    redeem_manager: Arc<RwLock<RedeemManager>>,
    storage_client: Arc<RwLock<StorageClient>>,
    role_cache: Arc<RwLock<RoleCache>>,
    user_links: Arc<UserLinks>,
    logger: Arc<Logger>,
    websocket_tx: mpsc::Sender<WebSocketMessage>,
    world_info: Arc<Mutex<Option<World>>>,
    vrchat_client: Arc<VRChatClient>, // Add this parameter
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting Twitch message handling...");

    let message_handler = MessageHandler::new(
        twitch_bot_client,
        config,
        api_client,
        redeem_manager,
        storage_client,
        role_cache,
        user_links,
        logger.clone(),
        websocket_tx,
        world_info,
        vrchat_client // Use the parameter here
    );

    message_handler.handle_messages().await?;

    println!("Twitch message handling ended.");
    Ok(())
}