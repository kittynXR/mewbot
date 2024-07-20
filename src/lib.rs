pub mod config;
pub mod twitch;
pub mod vrchat;
pub mod discord;
pub mod logging;
pub mod ai;
pub mod osc;

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::sync::mpsc::UnboundedReceiver;
use twitch_irc::message::ServerMessage;
use twitch_irc::TwitchIRCClient;
use twitch_irc::SecureTCPTransport;
use twitch_irc::login::StaticLoginCredentials;
use twitch_irc::ClientConfig;
use crate::config::Config;
use crate::twitch::api::TwitchAPIClient;
use crate::vrchat::VRChatClient;
use crate::vrchat::World;
use crate::twitch::TwitchEventSubClient;
use crate::twitch::eventsub::events::redemptions::RedemptionManager;
use crate::ai::AIClient;
use crate::osc::VRChatOSC;
use std::time::Duration;

pub struct BotClients {
    pub twitch_irc: Option<(Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>, UnboundedReceiver<ServerMessage>)>,
    pub twitch_api: Option<Arc<TwitchAPIClient>>, // Change this line
    pub vrchat: Option<VRChatClient>,
    pub discord: Option<()>, // Replace with DiscordClient when implemented
    pub redemption_manager: Arc<RedemptionManager>,
}

pub async fn init(config: Arc<RwLock<Config>>) -> Result<BotClients, Box<dyn std::error::Error + Send + Sync>> {
    let twitch_api = if config.read().await.is_twitch_api_configured() {
        let api_client = TwitchAPIClient::new(config.clone()).await?;
        api_client.authenticate().await?;
        Some(Arc::new(api_client))
    } else {
        None
    };

    let mut redemption_manager = Arc::new(RedemptionManager::new(
        None, // AI client
        None, // OSC client
        twitch_api.clone().ok_or("Twitch API client not initialized")?,
    ));

    // Initialize RedemptionManager
    {
        let manager = Arc::get_mut(&mut redemption_manager).unwrap();
        println!("Loading redemption settings...");
        if let Err(e) = manager.load_settings() {
            eprintln!("Failed to load redemption settings: {}. Using default settings.", e);
        }
        println!("Updating Twitch redeems based on local settings...");
        if let Err(e) = manager.update_twitch_redeems().await {
            eprintln!("Failed to update Twitch redeems: {}. Local settings may not be reflected on Twitch.", e);
        }
        println!("Updating local settings from Twitch...");
        if let Err(e) = manager.update_from_twitch().await {
            eprintln!("Failed to update redemption settings from Twitch: {}. Using local settings.", e);
        }

    }

    // Start periodic updates
    let mut update_manager = Arc::clone(&redemption_manager);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await; // Update every hour
            let manager = Arc::get_mut(&mut update_manager).unwrap();
            if let Err(e) = manager.update_from_twitch().await {
                eprintln!("Failed to update redemption settings: {}", e);
            }
        }
    });

    let mut clients = BotClients {
        twitch_irc: None,
        twitch_api,
        vrchat: None,
        discord: None,
        redemption_manager,
    };

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

        clients.twitch_irc = Some((client, incoming_messages));
        println!("Twitch IRC client initialized successfully.");
    } else {
        println!("Twitch IRC is not configured. Skipping initialization.");
    }

    if config.read().await.is_twitch_api_configured() {
        let twitch_api_client = TwitchAPIClient::new(config.clone()).await?;
        twitch_api_client.authenticate().await?;
        clients.twitch_api = Some(Arc::from(twitch_api_client));
        println!("Twitch API client initialized and authenticated successfully.");
    }

    match VRChatClient::new(config.clone()).await {
        Ok(vrchat_client) => {
            clients.vrchat = Some(vrchat_client);
            println!("VRChat client initialized successfully.");
        }
        Err(e) => {
            eprintln!("Failed to initialize VRChat client: {}. VRChat functionality will be disabled.", e);
        }
    }

    if config.read().await.discord_token.is_some() {
        println!("Discord client initialization not yet implemented.");
    }

    if clients.twitch_irc.is_none() && clients.vrchat.is_none() && clients.discord.is_none() {
        return Err("Bot requires at least one of Twitch IRC, VRChat, or Discord to be configured.".into());
    }

    Ok(clients)
}

pub async fn run(mut clients: BotClients, config: Arc<RwLock<Config>>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let world_info = Arc::new(Mutex::new(None::<World>));
    let mut handles = vec![];

    if let (Some((irc_client, incoming_messages)), Some(api_client)) = (clients.twitch_irc.take(), clients.twitch_api.take()) {
        println!("Setting up Twitch IRC message handling...");

        let world_info_clone = Arc::clone(&world_info);
        let api_client = Arc::new(api_client);

        let channel = config.read().await.twitch_channel_to_join.clone()
            .ok_or("Twitch channel to join not set")?;

        let irc_client_for_messages = Arc::clone(&irc_client);

        let twitch_handler = tokio::spawn({
            let api_client_clone = Arc::clone(&api_client);
            let config_clone = Arc::clone(&config);
            let redemption_manager_clone = Arc::clone(&clients.redemption_manager);
            async move {
                handle_twitch_messages(irc_client_for_messages, incoming_messages, world_info_clone, api_client_clone, config_clone, redemption_manager_clone).await
            }
        });
        handles.push(twitch_handler);
        println!("Twitch IRC handler started.");

        let config_clone = Arc::clone(&config);
        let eventsub_client = TwitchEventSubClient::new(
            config_clone,
            Arc::clone(&api_client),
            irc_client.clone(),
            channel.clone(),
            None, // AI client
            None  // OSC client
        );

        let eventsub_handle = tokio::spawn(async move {
            println!("Debug: Starting EventSub client");
            if let Err(e) = eventsub_client.connect_and_listen().await {
                eprintln!("EventSub client error: {:?}", e);
            }
            Ok(()) as Result<(), Box<dyn std::error::Error + Send + Sync>>
        });

        handles.push(eventsub_handle);
        println!("EventSub client started.");
    } else {
        println!("Twitch IRC client or API client not initialized. Skipping Twitch handlers setup.");
    }

    if let Some(vrchat_client) = clients.vrchat.take() {
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

    println!("Bot has shut down.");
    Ok(())
}

async fn handle_twitch_messages(
    client: Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    mut incoming_messages: UnboundedReceiver<ServerMessage>,
    world_info: Arc<Mutex<Option<World>>>,
    api_client: Arc<Arc<TwitchAPIClient>>,
    config: Arc<RwLock<Config>>,
    redemption_manager: Arc<RedemptionManager>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting Twitch message handling...");
    while let Some(message) = incoming_messages.recv().await {
        match &message {
            ServerMessage::Ping(_) | ServerMessage::Pong(_) => {
                log_verbose!(config, "Received IRC message: {:?}", message);
            }
            ServerMessage::Privmsg(msg) => {
                let client_clone = Arc::clone(&client);
                let world_info_clone = Arc::clone(&world_info);
                let api_client_clone = Arc::clone(&api_client);
                let config_clone = Arc::clone(&config);
                let redemption_manager_clone = Arc::clone(&redemption_manager);
                let msg_clone = msg.clone();

                tokio::spawn(async move {
                    if let Err(e) = crate::twitch::irc::handler::handle_twitch_message(
                        &msg_clone,
                        client_clone,
                        world_info_clone,
                        api_client_clone,
                        config_clone,
                        redemption_manager_clone
                    ).await {
                        eprintln!("Error handling Twitch message: {:?}", e);
                    }
                });
            }
            _ => println!("Received other IRC message: {:?}", message),
        }
    }
    println!("Twitch message handling ended.");
    Ok(())
}