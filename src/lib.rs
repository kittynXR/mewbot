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
use tokio::sync::mpsc::UnboundedReceiver;
use twitch_irc::message::ServerMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::ClientConfig;
use crate::twitch::irc::TwitchIRCClient as CustomTwitchIRCClient;

use crate::config::Config;
use crate::twitch::api::TwitchAPIClient;
use crate::vrchat::VRChatClient;
use crate::vrchat::World;
use crate::twitch::TwitchEventSubClient;
use crate::twitch::redeems::RedeemManager;
use crate::ai::AIClient;
//use crate::osc::VRChatOSC;
use std::time::Duration;
// use tokio::io::AsyncWriteExt;
use tokio::task::{JoinHandle, spawn_blocking};
use tokio::time::timeout;
use crate::discord::UserLinks;
use crate::logging::{LogLevel, Logger};
use crate::osc::VRChatOSC;
use crate::osc::osc_config::OSCConfigurations;
use crate::storage::StorageClient;


pub struct BotClients {
    pub twitch_irc: Option<(Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>, UnboundedReceiver<ServerMessage>)>,
    pub twitch_api: Option<Arc<TwitchAPIClient>>,
    pub vrchat: Option<VRChatClient>,  // Make sure this is the correct type
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
}

impl BotClients {
    pub async fn shutdown(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Initiating graceful shutdown...");

        // Notify users about shutdown
        self.notify_shutdown().await?;

        // Gracefully stop Twitch IRC client
        if let Some((irc_client, _)) = &self.twitch_irc {
            println!("Stopping Twitch IRC client...");
            match timeout(Duration::from_secs(5), async {
                // Send a final message to the channel
                let channel = self.get_twitch_channel()?;
                irc_client.say(channel, "MewBot is shutting down. Goodbye!".to_string()).await?;

                // Sleep briefly to allow the message to be sent
                tokio::time::sleep(Duration::from_millis(500)).await;

                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
            }).await {
                Ok(result) => {
                    if let Err(e) = result {
                        eprintln!("Error stopping Twitch IRC client: {}", e);
                    } else {
                        println!("Twitch IRC client stopped successfully");
                    }
                },
                Err(_) => eprintln!("Timeout while stopping Twitch IRC client"),
            }
        }

        // Disconnect from VRChat
        if let Some(vrchat_client) = &mut self.vrchat {
            println!("Disconnecting from VRChat...");
            match timeout(Duration::from_secs(5), vrchat_client.disconnect()).await {
                Ok(result) => {
                    if let Err(e) = result {
                        eprintln!("Error disconnecting from VRChat: {}", e);
                    }
                },
                Err(_) => eprintln!("Timeout while disconnecting from VRChat"),
            }
        }

        // Save final state
        println!("Saving final redemption settings...");
        if let Err(e) = self.redeem_manager.read().await.save_settings().await {
            eprintln!("Error saving redemption settings: {}", e);
        }

        println!("Shutdown complete.");
        self.bot_status.write().await.set_online(false);
        Ok(())
    }

    async fn notify_shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some((irc_client, _)) = &self.twitch_irc {
            let channel = self.get_twitch_channel()?;
            irc_client.say(channel, "MewBot is shutting down. Thank you for using our services!".to_string()).await?;
        }
        Ok(())
    }

    fn get_twitch_channel(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Implement this method to return the correct Twitch channel
        // For now, we'll return a placeholder
        Ok("kittyn".to_string())
    }

    async fn save_final_state(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement saving final state here
        // For example:
        if let Err(e) = self.redeem_manager.read().await.save_settings().await {
            eprintln!("Error saving RedeemManager state: {}", e);
        }

        // Add any other state saving logic here

        Ok(())
    }
}

pub async fn init(config: Arc<RwLock<Config>>) -> Result<BotClients, Box<dyn std::error::Error + Send + Sync>> {
    let bot_status = BotStatus::new();

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

    let vrchat = match VRChatClient::new(config.clone()).await {
        Ok(vrchat_client) => {
            println!("VRChat client initialized successfully.");
            Some(vrchat_client)
        }
        Err(e) => {
            eprintln!("Failed to initialize VRChat client: {}. VRChat functionality will be disabled.", e);
            None
        }
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

    let mut twitch_irc = None;
    if config.read().await.is_twitch_irc_configured() {
        println!("Initializing Twitch IRC client...");

        let config_read = config.read().await;
        let username = config_read.twitch_bot_username.clone().ok_or("Twitch IRC username not set")?;
        let oauth_token = config_read.twitch_irc_oauth_token.clone().ok_or("Twitch IRC OAuth token not set")?;
        let channel = config_read.twitch_channel_to_join.clone().ok_or("Twitch channel to join not set")?;

        println!("Twitch IRC username: {}", username);
        println!("Twitch IRC OAuth token (first 10 chars): {}...", &oauth_token[..10]);
        println!("Twitch channel to join: {}", channel);

        let oauth_token = oauth_token.trim_start_matches("oauth:").to_string();

        let client_config = ClientConfig::new_simple(
            StaticLoginCredentials::new(username, Some(oauth_token))
        );

        println!("Creating Twitch IRC client...");
        let (incoming_messages, client) = TwitchIRCClient::<SecureTCPTransport, StaticLoginCredentials>::new(client_config);

        let client = Arc::new(client);

        println!("Joining Twitch channel...");
        client.join(channel.clone())?;
        println!("Successfully joined channel: {}", channel);

        twitch_irc = Some((client, incoming_messages));
        println!("Twitch IRC client initialized successfully.");
    } else {
        println!("Twitch IRC is not configured. Skipping initialization.");
    }

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

    let osc_configs = Arc::new(RwLock::new(OSCConfigurations::load("osc_config.json").unwrap_or_default()));
    let logger = Arc::new(Logger::new(config.clone()));

    let eventsub_client = if let (Some(ref irc_client), Some(ref api_client)) = (twitch_irc.as_ref(), &twitch_api) {
        let channel = config.read().await.twitch_channel_to_join.clone().ok_or("Twitch channel to join not set")?;
        Arc::new(Mutex::new(TwitchEventSubClient::new(
            config.clone(),
            Arc::clone(api_client),
            Arc::clone(&irc_client.0),
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

    let storage = Arc::new(RwLock::new(StorageClient::new("mewbot_data.db")?));
    let role_cache = Arc::new(RwLock::new(RoleCache::new()));

    let clients = BotClients {
        twitch_irc,
        twitch_api,
        vrchat,
        discord,
        redeem_manager,
        ai_client,
        eventsub_client: eventsub_client.clone(),
        vrchat_osc,
        storage,
        role_cache,
        user_links,
        logger: logger.clone(),
        bot_status,
    };

    if clients.twitch_irc.is_none() && clients.vrchat.is_none() && clients.discord.is_none() {
        return Err("Bot requires at least one of Twitch IRC, VRChat, or Discord to be configured.".into());
    }

    // Reset coin game and update redeems based on stream status

    clients.redeem_manager.write().await.reset_coin_game().await?;
    // clients.redeem_manager.write().await.update_stream_status(false, "".to_string()).await;
    let eventsub_client_clone = eventsub_client.clone();
    tokio::spawn(async move {
        let client = eventsub_client_clone.lock().await;
        if let Err(e) = client.check_current_stream_status().await {
            eprintln!("Failed to check current stream status: {:?}", e);
        }
    });

    clients.bot_status.write().await.set_online(true);

    Ok(clients)
}

pub async fn run(clients: BotClients, config: Arc<RwLock<Config>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut handles: Vec<JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>> = vec![];

    let web_ui = Arc::new(web_ui::WebUI::new(
        config.clone(),
        clients.storage.clone(),
        clients.logger.clone(),
        clients.bot_status.clone(),
        clients.twitch_irc.as_ref().map(|(client, _)| {
            Arc::new(CustomTwitchIRCClient {
                client: client.clone(),
                message_receiver: tokio::sync::mpsc::unbounded_channel().1, // Create a dummy receiver
            })
        }).unwrap_or_else(|| panic!("Twitch IRC client not initialized")),
        clients.vrchat_osc.clone(),
    ));
    let web_ui_clone = web_ui.clone();

    let web_ui_handle = tokio::spawn(async move {
        web_ui_clone.run().await;
    });


    let world_info = Arc::new(Mutex::new(None::<World>));
    // let mut handles = vec![];

    // Initialize redeems
    println!("Initializing channel point redeems...");
    if let Err(e) = clients.redeem_manager.write().await.initialize_redeems().await {
        eprintln!("Failed to initialize channel point redeems: {}. Some redeems may not be available.", e);
    }

    if let (Some((irc_client, incoming_messages)), Some(api_client)) = (clients.twitch_irc, clients.twitch_api) {
        println!("Setting up Twitch IRC message handling...");

        let world_info_clone = Arc::clone(&world_info);
        let api_client = Arc::new(api_client);

        let channel = config.read().await.twitch_channel_to_join.clone()
            .ok_or("Twitch channel to join not set")?;

        // Announce redeems after initialization
        if let Err(e) = clients.redeem_manager.read().await.announce_redeems(&irc_client, &channel).await {
            eprintln!("Failed to announce redeems: {}", e);
        }

        let irc_client_for_messages = Arc::clone(&irc_client);

        let twitch_handler = tokio::spawn({
            let api_client_clone = Arc::clone(&api_client);
            let config_clone = Arc::clone(&config);
            let redeem_manager_clone = Arc::clone(&clients.redeem_manager);
            let vrchat_osc_clone = clients.vrchat_osc.clone();
            let storage_clone = Arc::clone(&clients.storage);
            let role_cache_clone = Arc::clone(&clients.role_cache);
            let user_links_clone = Arc::clone(&clients.user_links);
            let logger_clone = clients.logger.clone(); // Use clients.logger here
            async move {
                handle_twitch_messages(
                    irc_client_for_messages,
                    incoming_messages,
                    world_info_clone,
                    api_client_clone,
                    config_clone,
                    redeem_manager_clone,
                    vrchat_osc_clone,
                    storage_clone,
                    role_cache_clone,
                    user_links_clone,
                    logger_clone,
                ).await
            }
        });
        handles.push(twitch_handler);
        println!("Twitch IRC handler started.");
        println!("Checking RedeemManager initialization...");
        let handler_count = clients.redeem_manager.read().await.get_handler_count().await;
        println!("Number of initialized handlers: {}", handler_count);
        println!("Twitch IRC handler started.");
    }

    if let Some(discord_client) = clients.discord {
        let discord_handle: JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> = tokio::spawn(async move {
            match Arc::try_unwrap(discord_client) {
                Ok(client) => {
                    if let Err(e) = client.start().await {
                        eprintln!("Discord client error: {:?}", e);
                        return Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>);
                    }
                },
                Err(_) => {
                    eprintln!("Failed to unwrap Arc, Discord client is still in use elsewhere");
                    return Err(Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to unwrap Arc")) as Box<dyn std::error::Error + Send + Sync>);
                }
            }
            Ok(())
        });
        handles.push(discord_handle);
    }

    // EventSub client handling
    let eventsub_client = clients.eventsub_client.clone();
    let eventsub_handle = tokio::spawn(async move {
        println!("Debug: Starting EventSub client");
        if let Err(e) = eventsub_client.lock().await.connect_and_listen().await {
            eprintln!("EventSub client error: {:?}", e);
        }
        Ok(()) as Result<(), Box<dyn std::error::Error + Send + Sync>>
    });

    handles.push(eventsub_handle);
    println!("EventSub client started.");

    // Start the token refresh task
    let eventsub_client_clone = clients.eventsub_client.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await; // 1 hour
            if let Err(e) = eventsub_client_clone.lock().await.refresh_token_periodically().await {
                eprintln!("Failed to refresh token: {:?}", e);
            }
        }
    });

    if let Some(vrchat_client) = clients.vrchat {
        let current_user_id = vrchat_client.get_current_user_id().await?;
        let auth_cookie = vrchat_client.get_auth_cookie().await;
        let vrchat_handle = tokio::spawn(async move {
            if let Err(e) = crate::vrchat::websocket::handler(auth_cookie, world_info.clone(), current_user_id).await {
                eprintln!("VRChat websocket handler error: {:?}", e);
            }
            Ok(()) as Result<(), Box<dyn std::error::Error + Send + Sync>>
        });
        handles.push(vrchat_handle);
        println!("VRChat websocket handler started.");
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
    client: Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    mut incoming_messages: UnboundedReceiver<ServerMessage>,
    world_info: Arc<Mutex<Option<World>>>,
    api_client: Arc<Arc<TwitchAPIClient>>,
    config: Arc<RwLock<Config>>,
    redeem_manager: Arc<RwLock<RedeemManager>>,
    vrchat_osc: Option<Arc<VRChatOSC>>,
    storage_client: Arc<RwLock<StorageClient>>,
    role_cache: Arc<RwLock<RoleCache>>,
    user_links: Arc<UserLinks>,
    logger: Arc<Logger>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting Twitch message handling...");
    while let Some(message) = incoming_messages.recv().await {
        match &message {
            ServerMessage::Privmsg(msg) => {
                let client_clone = Arc::clone(&client);
                let world_info_clone = Arc::clone(&world_info);
                let api_client_clone = Arc::clone(&api_client);
                let config_clone = Arc::clone(&config);
                let redeem_manager_clone = Arc::clone(&redeem_manager);
                let vrchat_osc_clone = vrchat_osc.clone();
                let msg_clone = msg.clone();
                let storage_clone = storage_client.clone();
                let role_cache_clone = role_cache.clone();
                let user_links_clone = user_links.clone();
                let logger_clone = logger.clone();

                tokio::spawn(async move {
                    let handle_result = crate::twitch::irc::handler::handle_twitch_message(
                        &msg_clone,
                        client_clone,
                        world_info_clone,
                        api_client_clone,
                        config_clone,
                        redeem_manager_clone,
                        vrchat_osc_clone,
                        storage_clone,
                        role_cache_clone,
                        user_links_clone,
                        logger_clone.clone(), // Clone here
                    ).await;

                    if let Err(e) = handle_result {
                        log_error!(logger_clone, "Error handling Twitch message: {:?}", e);
                    }
                });
            }
            _ => {
                log_debug!(logger, "Received other IRC message: {:?}", message);
            }
        }
    }
    println!("Twitch message handling ended.");
    Ok(())
}