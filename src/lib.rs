pub mod config;
pub mod twitch;
pub mod vrchat;
pub mod discord;

use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio::sync::mpsc::{self, UnboundedReceiver};
//use futures::StreamExt;
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

pub struct BotClients {
    pub twitch_irc: Option<(Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>, UnboundedReceiver<ServerMessage>)>,
    pub twitch_api: Option<TwitchAPIClient>,
    pub vrchat: Option<VRChatClient>,
    pub discord: Option<()>, // Replace with DiscordClient when implemented
}

pub async fn init(config: Arc<RwLock<Config>>) -> Result<BotClients, Box<dyn std::error::Error + Send + Sync>> {
    let mut clients = BotClients {
        twitch_irc: None,
        twitch_api: None,
        vrchat: None,
        discord: None,
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

        // Ensure the OAuth token does NOT start with "oauth:"
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

    // Initialize Twitch API client
    if config.read().await.is_twitch_api_configured() {
        let twitch_api_client = TwitchAPIClient::new(config.clone()).await?;
        twitch_api_client.authenticate().await?;
        clients.twitch_api = Some(twitch_api_client);
        println!("Twitch API client initialized and authenticated successfully.");
    }

    // Initialize VRChat client
    match VRChatClient::new(config.clone()).await {
        Ok(vrchat_client) => {
            clients.vrchat = Some(vrchat_client);
            println!("VRChat client initialized successfully.");
        }
        Err(e) => {
            eprintln!("Failed to initialize VRChat client: {}. VRChat functionality will be disabled.", e);
        }
    }

    // Initialize Discord client (if implemented)
    if config.read().await.discord_token.is_some() {
        // clients.discord = Some(DiscordClient::new(config.clone()).await?);
        println!("Discord client initialization not yet implemented.");
    }

    if clients.twitch_irc.is_none() && clients.vrchat.is_none() && clients.discord.is_none() {
        return Err("Bot requires at least one of Twitch IRC, VRChat, or Discord to be configured.".into());
    }

    Ok(clients)
}

pub async fn run(mut clients: BotClients) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let world_info = Arc::new(Mutex::new(None::<World>));
    let mut handles = vec![];

    // Handle Twitch IRC and API
    if let (Some((irc_client, incoming_messages)), Some(api_client)) = (clients.twitch_irc.take(), clients.twitch_api.take()) {
        println!("Setting up Twitch IRC message handling...");

        let world_info_clone = Arc::clone(&world_info);
        let api_client = Arc::new(api_client);

        // Get the channel name from the configuration
        let config = Arc::new(RwLock::new(Config::new()?));
        let channel = config.read().await.twitch_channel_to_join.clone()
            .ok_or("Twitch channel to join not set")?;

        // Clone irc_client for use in both async blocks
        let irc_client_for_messages = Arc::clone(&irc_client);

        // Twitch IRC handler
        let twitch_handler = tokio::spawn({
            let api_client_clone = Arc::clone(&api_client);
            async move {
                handle_twitch_messages(irc_client_for_messages, incoming_messages, world_info_clone, api_client_clone).await
            }
        });
        handles.push(twitch_handler);
        println!("Twitch IRC handler started.");

        // Twitch EventSub handler
        let eventsub_client = TwitchEventSubClient::new(
            config.clone(),
            api_client.clone(),
            irc_client,  // Use the original irc_client here
            channel
        );
        let eventsub_handle = tokio::spawn(async move {
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

    // Handle VRChat
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

    // Wait for Ctrl+C or for all handles to complete
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
    api_client: Arc<TwitchAPIClient>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting Twitch message handling...");
    while let Some(message) = incoming_messages.recv().await {
        println!("Received IRC message: {:?}", message);
        match message {
            ServerMessage::Privmsg(msg) => {
                let client_clone = Arc::clone(&client);
                let world_info_clone = Arc::clone(&world_info);
                let api_client_clone = Arc::clone(&api_client);
                tokio::spawn(async move {
                    if let Err(e) = crate::twitch::irc::handler::handle_twitch_message(
                        msg,
                        client_clone,
                        world_info_clone,
                        api_client_clone
                    ).await {
                        eprintln!("Error handling Twitch message: {:?}", e);
                    }
                });
            }
            ServerMessage::Notice(notice) => {
                if notice.message_text.contains("Login authentication failed") {
                    eprintln!("Twitch IRC authentication failed. Please check your OAuth token.");
                } else {
                    println!("Received notice: {:?}", notice);
                }
            }
            _ => {
                println!("Received other IRC message: {:?}", message);
            }
        }
    }
    println!("Twitch message handling ended.");
    Ok(())
}

// async fn forward_messages(
//     mut incoming_messages: UnboundedReceiver<ServerMessage>,
//     tx: mpsc::Sender<twitch_irc::message::PrivmsgMessage>,
// ) {
//     println!("Message forwarding started.");
//     while let Some(message) = incoming_messages.recv().await {
//         println!("Received IRC message: {:?}", message);
//         if let ServerMessage::Privmsg(msg) = message {
//             if tx.send(msg).await.is_err() {
//                 eprintln!("Failed to send Twitch message to channel");
//                 break;
//             }
//         }
//     }
//     println!("Message forwarding ended.");
// }