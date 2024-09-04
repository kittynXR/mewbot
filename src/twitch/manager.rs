use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::error::Error as StdError;
use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use futures_util::TryFutureExt;
use log::{debug, error, info, warn};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::time::timeout;
use crate::ai::AIClient;
use crate::config::Config;
use crate::discord::UserLinks;
use crate::osc::osc_config::OSCConfigurations;
use crate::osc::{OSCManager, VRChatOSC};
use crate::osc::client::OSCClient;
use crate::storage::{ChatterData, StorageClient};
use crate::twitch::{TwitchAPIClient, TwitchIRCManager};
use crate::twitch::eventsub::TwitchEventSubClient;
use crate::twitch::irc::{MessageHandler, TwitchBotClient, TwitchBroadcasterClient};
use crate::twitch::redeems::RedeemManager;
use crate::twitch::roles::UserRole;
use crate::web_ui::websocket::{DashboardState, WebSocketMessage};
use crate::twitch::irc::commands::ad_commands::AdManager;
use crate::twitch::irc::commands::shoutout::ShoutoutCooldown;

use std::fmt::Debug;
use crate::twitch::api::client::TwitchAPIError;

#[derive(Clone, Debug)]
pub struct TwitchUser {
    pub user_id: String,
    pub username: String,
    pub display_name: String,
    pub role: UserRole,
    pub last_seen: DateTime<Utc>,
    pub last_role_check: DateTime<Utc>,
    pub messages: VecDeque<(DateTime<Utc>, String)>,
    pub streamer_data: Option<StreamerData>,
}

#[derive(Clone, Debug)]
pub struct StreamerData {
    pub recent_games: Vec<String>,
    pub current_tags: Vec<String>,
    pub current_title: String,
    pub recent_titles: VecDeque<String>,
    pub top_clips: Vec<(String, String)>, // (clip_title, clip_url)
}

pub struct UserManager {
    user_cache: Arc<RwLock<HashMap<String, TwitchUser>>>,
    api_client: Arc<TwitchAPIClient>,
}

impl From<TwitchUser> for ChatterData {
    fn from(user: TwitchUser) -> Self {
        ChatterData {
            user_id: user.user_id,
            username: user.username,
            messages: vec![],
            sentiment: 0.0,
            chatter_type: "".to_string(),
            is_streamer: false,
            stream_titles: None,
            stream_categories: None,
            content_summary: None,
            role: user.role,
            last_seen: user.last_seen,
            // Add any other fields that ChatterData might have
            custom_notes: None,
        }
    }
}
impl UserManager {
    pub fn new(api_client: Arc<TwitchAPIClient>) -> Self {
        Self {
            user_cache: Arc::new(RwLock::new(HashMap::new())),
            api_client,
        }
    }

    pub async fn get_user(&self, user_id: &str) -> Result<TwitchUser, Box<dyn std::error::Error + Send + Sync>> {
        debug!("Attempting to get user with ID: {}", user_id);
        let cache = self.user_cache.read().await;

        if let Some(user) = cache.get(user_id) {
            debug!("User found in cache: {:?}", user);
            return Ok(user.clone());
        }
        drop(cache);

        debug!("User not found in cache. Fetching from API...");
        match self.api_client.get_user_info_by_id(user_id).await {
            Ok(user_info) => {
                debug!("API response for user info: {:?}", user_info);

                let user_data = user_info["data"].as_array()
                    .and_then(|arr| arr.first())
                    .ok_or_else(|| {
                        error!("User data not found in API response");
                        Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "User data not found"))
                    })?;

                let username = user_data["login"].as_str()
                    .ok_or_else(|| {
                        error!("Username not found in API response");
                        Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Username not found"))
                    })?
                    .to_string();
                let display_name = user_data["display_name"].as_str().unwrap_or(&username).to_string();

                let user = TwitchUser {
                    user_id: user_id.to_string(),
                    username,
                    display_name,
                    role: UserRole::Viewer, // Default role, will be updated later if needed
                    last_seen: Utc::now(),
                    last_role_check: Utc::now(),
                    messages: VecDeque::new(),
                    streamer_data: None,
                };

                debug!("New user created: {:?}", user);
                let mut cache = self.user_cache.write().await;
                cache.insert(user_id.to_string(), user.clone());
                Ok(user)
            },
            Err(e) => {
                error!("Failed to fetch user info from API: {:?}", e);
                Err(Box::new(e))  // Box the error here
            }
        }
    }

    async fn fetch_user_role(&self, user_id: &str) -> Result<UserRole, Box<dyn std::error::Error + Send + Sync>> {
        let channel_id = self.api_client.get_broadcaster_id().await?;

        if user_id == channel_id {
            return Ok(UserRole::Broadcaster);
        }

        if self.api_client.check_user_mod(&channel_id, user_id).await? {
            return Ok(UserRole::Moderator);
        }

        if self.api_client.check_user_vip(&channel_id, user_id).await? {
            return Ok(UserRole::VIP);
        }

        if self.api_client.check_user_subscription(&channel_id, user_id).await? {
            return Ok(UserRole::Subscriber);
        }

        Ok(UserRole::Viewer)
    }

    pub async fn update_user(&self, user_id: String, user: TwitchUser) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cache = self.user_cache.write().await;
        cache.insert(user_id, user);
        Ok(())
    }

    pub async fn update_user_role(&self, user_id: &str, role: UserRole) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cache = self.user_cache.write().await;
        if let Some(user) = cache.get_mut(user_id) {
            user.role = role.clone();
            user.last_role_check = Utc::now();
            info!("Updated role for user {}: {:?}", user_id, role);
        } else {
            error!("Attempted to update role for non-existent user: {}", user_id);
        }
        Ok(())
    }

    pub async fn add_user_message(&self, user_id: &str, message: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cache = self.user_cache.write().await;
        if let Some(user) = cache.get_mut(user_id) {
            user.messages.push_front((Utc::now(), message));
            if user.messages.len() > 100 {
                user.messages.pop_back();
            }
            user.last_seen = Utc::now();
        } else {
            // If the user doesn't exist, create a new user entry
            let new_user = TwitchUser {
                user_id: user_id.to_string(),
                username: String::new(), // We'll need to fetch this from the API
                display_name: String::new(),
                role: UserRole::Viewer, // Default role
                last_seen: Utc::now(),
                last_role_check: Utc::now(),
                messages: VecDeque::from([(Utc::now(), message)]),
                streamer_data: None,
            };
            cache.insert(user_id.to_string(), new_user);
        }
        Ok(())
    }

    pub async fn refresh_roles(&self) {
        let users_to_refresh = {
            let cache = self.user_cache.read().await;
            cache.keys().cloned().collect::<Vec<String>>()
        };

        for user_id in users_to_refresh {
            match self.fetch_user_role(&user_id).await {
                Ok(role) => {
                    if let Err(e) = self.update_user_role(&user_id, role).await {
                        error!("Failed to update role for user {}: {:?}", user_id, e);
                    }
                }
                Err(e) => {
                    error!("Failed to fetch role for user {}: {:?}", user_id, e);
                }
            }
        }
    }

    pub async fn start_role_refresh_task(&self) {
        let user_manager = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(900)).await; // Run every 15 minutes
                user_manager.refresh_roles().await;
            }
        });
    }

    // Add this method to update streamer data
    pub async fn update_streamer_data(&self, user_id: &str, streamer_data: StreamerData) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut cache = self.user_cache.write().await;
        if let Some(user) = cache.get_mut(user_id) {
            user.streamer_data = Some(streamer_data);
            debug!("Updated streamer data for user {}", user_id);
        } else {
            error!("Attempted to update streamer data for non-existent user: {}", user_id);
        }
        Ok(())
    }
}

impl Clone for UserManager {
    fn clone(&self) -> Self {
        UserManager {
            user_cache: self.user_cache.clone(),
            api_client: self.api_client.clone(),
        }
    }
}

#[derive(Clone)]
pub struct TwitchManager {
    pub config: Arc<Config>,
    pub api_client: Arc<TwitchAPIClient>,
    pub irc_manager: Arc<TwitchIRCManager>,
    pub bot_client: Arc<TwitchBotClient>,
    pub broadcaster_client: Option<Arc<TwitchBroadcasterClient>>,
    pub redeem_manager: Arc<RwLock<RedeemManager>>,
    pub eventsub_client: Arc<Mutex<Option<TwitchEventSubClient>>>,
    pub user_manager: UserManager,
    pub(crate) user_links: Arc<UserLinks>,
    stream_status: Arc<RwLock<bool>>,
    pub vrchat_osc: Option<Arc<VRChatOSC>>,
    pub ai_client: Option<Arc<AIClient>>,
    shoutout_queue: Arc<Mutex<VecDeque<(String, String)>>>, // (user_id, username)
    shoutout_last_processed: Arc<Mutex<Instant>>,
    pub shoutout_cooldowns: Arc<Mutex<ShoutoutCooldown>>,
    shoutout_sender: mpsc::Sender<(String, String)>,
    pub ad_manager: Arc<RwLock<AdManager>>,
}

impl TwitchManager {
    pub async fn new(
        config: Arc<Config>,
        storage: Arc<RwLock<StorageClient>>,
        ai_client: Option<Arc<AIClient>>,
        vrchat_osc: Option<Arc<VRChatOSC>>,
        user_links: Arc<UserLinks>,
        dashboard_state: Arc<RwLock<DashboardState>>,
        websocket_tx: mpsc::UnboundedSender<WebSocketMessage>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        let api_client = Arc::new(TwitchAPIClient::new(config.clone()).await?);
        api_client.authenticate().await?;

        let social_links = config.social_links.clone();
        let irc_manager = Arc::new(TwitchIRCManager::new(
            websocket_tx.clone(),
            Arc::new(RwLock::new(social_links)),
            dashboard_state.clone(),
            config.clone(),
        ));

        let (bot_client, broadcaster_client) = Self::initialize_irc_clients(&config, &irc_manager).await?;

        let osc_configs = Arc::new(RwLock::new(OSCConfigurations::load("osc_config.json").unwrap_or_default()));
        let osc_manager = Arc::new(OSCManager::new("127.0.0.1:9000").await?);
        
        let vrchat_osc = Self::initialize_vrchat_osc(vrchat_osc).await;

        let redeem_manager = Arc::new(RwLock::new(RedeemManager::new(
            api_client.clone(),
            ai_client.clone().unwrap_or_else(|| Arc::new(AIClient::new(None, None))),
            osc_manager.clone(),
            osc_configs.clone(),
        )));

        let user_manager = UserManager::new(api_client.clone());

        let shoutout_queue = Arc::new(Mutex::new(VecDeque::new()));
        let shoutout_last_processed = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(120)));
        let shoutout_cooldowns = Arc::new(Mutex::new(ShoutoutCooldown::new()));
        let (shoutout_sender, shoutout_receiver) = mpsc::channel(100);

        let ad_manager = Arc::new(RwLock::new(AdManager::new()));

        let mut twitch_manager = Self {
            config,
            api_client,
            irc_manager,
            bot_client,
            broadcaster_client,
            redeem_manager,
            eventsub_client: Arc::new(Mutex::new(None)),
            user_manager,
            user_links,
            stream_status: Arc::new(RwLock::new(false)),
            vrchat_osc: vrchat_osc.clone(),
            ai_client: ai_client.clone(),
            shoutout_queue,
            shoutout_last_processed,
            shoutout_cooldowns,
            shoutout_sender,
            ad_manager,
        };

        let eventsub_client = Self::initialize_eventsub_client(
            &twitch_manager.config,
            &twitch_manager.api_client,
            &twitch_manager.bot_client,
            &twitch_manager.redeem_manager,
            ai_client,
            vrchat_osc,
            &Arc::new(twitch_manager.clone()),
        ).await?;

        twitch_manager.eventsub_client = Arc::new(Mutex::new(Some(eventsub_client)));

        twitch_manager.check_initial_stream_status().await?;

        // Start the shoutout queue processor
        let manager_clone = Arc::new(twitch_manager.clone());
        tokio::spawn(async move {
            manager_clone.process_shoutout_queue(shoutout_receiver).await;
        });

        Ok(twitch_manager)
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn StdError + Send + Sync>> {
        info!("Shutting down TwitchManager...");

        let shutdown_timeout = Duration::from_secs(15);

        // Shutdown IRC manager
        if let Err(e) = timeout(shutdown_timeout, self.irc_manager.shutdown()).await {
            error!("IRC manager shutdown timed out: {:?}", e);
        }

        // Shutdown EventSub client
        if let Some(eventsub_client) = self.eventsub_client.lock().await.as_ref() {
            match timeout(shutdown_timeout, eventsub_client.shutdown()).await {
                Ok(Ok(_)) => info!("EventSub client shut down successfully"),
                Ok(Err(e)) => error!("Error shutting down EventSub client: {:?}", e),
                Err(_) => error!("EventSub client shutdown timed out"),
            }
        }

        // Shutdown RedeemManager
        if let Err(e) = timeout(shutdown_timeout, self.redeem_manager.write().await.shutdown()).await {
            error!("RedeemManager shutdown timed out: {:?}", e);
        }

        info!("TwitchManager shutdown complete.");
        Ok(())
    }

    async fn initialize_vrchat_osc(existing_osc: Option<Arc<VRChatOSC>>) -> Option<Arc<VRChatOSC>> {
        if let Some(osc) = existing_osc {
            return Some(osc);
        }

        match OSCManager::new("127.0.0.1:9000").await {
            Ok(manager) => Some(manager.get_vrchat_osc()),
            Err(e) => {
                error!("Failed to create OSCManager: {:?}", e);
                match OSCClient::new("127.0.0.1:9000").await {
                    Ok(client) => Some(Arc::new(VRChatOSC::new(Arc::new(RwLock::new(client))))),
                    Err(e) => {
                        error!("Failed to create OSCClient: {:?}", e);
                        None
                    }
                }
            }
        }
    }

    async fn initialize_irc_clients(
        config: &Arc<Config>,
        irc_manager: &Arc<TwitchIRCManager>,
    ) -> Result<(Arc<TwitchBotClient>, Option<Arc<TwitchBroadcasterClient>>), Box<dyn std::error::Error + Send + Sync>> {
        let bot_username = config.twitch_bot_username.as_ref().ok_or("Twitch IRC bot username not set")?;
        let bot_oauth_token = config.twitch_bot_oauth_token.as_ref().ok_or("Bot OAuth token not set")?;
        let broadcaster_username = config.twitch_channel_to_join.as_ref().ok_or("Twitch channel to join not set")?;
        let channel = broadcaster_username.clone();

        irc_manager.add_client(bot_username.clone(), bot_oauth_token.clone(), vec![channel.clone()], true).await?;
        let bot_client = Arc::new(TwitchBotClient::new(bot_username.clone(), irc_manager.clone()));

        let broadcaster_client = if let Some(broadcaster_oauth_token) = &config.twitch_broadcaster_oauth_token {
            irc_manager.add_client(broadcaster_username.clone(), broadcaster_oauth_token.clone(), vec![channel.clone()], false).await?;
            Some(Arc::new(TwitchBroadcasterClient::new(broadcaster_username.clone(), irc_manager.clone())))
        } else {
            None
        };

        Ok((bot_client, broadcaster_client))
    }

    async fn initialize_eventsub_client(
        config: &Arc<Config>,
        api_client: &Arc<TwitchAPIClient>,
        bot_client: &Arc<TwitchBotClient>,
        redeem_manager: &Arc<RwLock<RedeemManager>>,
        ai_client: Option<Arc<AIClient>>,
        vrchat_osc: Option<Arc<VRChatOSC>>,
        twitch_manager: &Arc<TwitchManager>,
    ) -> Result<TwitchEventSubClient, Box<dyn std::error::Error + Send + Sync>> {
        let channel = config.twitch_channel_to_join.as_ref().ok_or("Twitch channel to join not set")?;

        let osc_configs = Arc::new(RwLock::new(OSCConfigurations::load("osc_config.json").unwrap_or_default()));

        Ok(TwitchEventSubClient::new(twitch_manager.clone(), channel.clone(), osc_configs))
    }

    pub async fn start_message_handler(&self, message_handler: Arc<MessageHandler>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut receiver = self.irc_manager.subscribe();

        tokio::spawn(async move {
            while let Ok(message) = receiver.recv().await {
                if let Err(e) = message_handler.handle_message(message).await {
                    error!("Error handling message: {:?}", e);
                }
            }
        });

        Ok(())
    }

    async fn check_initial_stream_status(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let channel_id = self.api_client.get_broadcaster_id().await?;
        let is_live = self.api_client.is_stream_live(&channel_id).await?;
        *self.stream_status.write().await = is_live;
        Ok(())
    }

    pub async fn is_stream_live(&self) -> bool {
        *self.stream_status.read().await
    }

    pub async fn set_stream_live(&self, is_live: bool) {
        *self.stream_status.write().await = is_live;
        // self.redeem_manager.write().await.set_stream_live(is_live).await.unwrap_or_else(|e| {
        //     error!("Failed to set stream status in redeem manager: {}", e);
        // });
    }

    pub async fn get_user(&self, user_id: &str) -> Result<TwitchUser, Box<dyn std::error::Error + Send + Sync>> {
        self.user_manager.get_user(user_id).await
    }

    pub fn get_vrchat_osc(&self) -> Option<Arc<VRChatOSC>> {
        self.vrchat_osc.clone()
    }

    pub fn get_bot_client(&self) -> Arc<TwitchBotClient> {
        self.bot_client.clone()
    }

    pub fn get_broadcaster_client(&self) -> Option<Arc<TwitchBroadcasterClient>> {
        self.broadcaster_client.clone()
    }

    pub fn get_redeem_manager(&self) -> Arc<RwLock<RedeemManager>> {
        self.redeem_manager.clone()
    }

    pub fn get_ai_client(&self) -> Option<Arc<AIClient>> {
        self.ai_client.clone()
    }

    pub fn get_api_client(&self) -> Arc<TwitchAPIClient> {
        self.api_client.clone()
    }

    pub fn get_ad_manager(&self) -> Arc<RwLock<AdManager>> {
        self.ad_manager.clone()
    }

    pub fn get_shoutout_cooldowns(&self) -> Arc<Mutex<ShoutoutCooldown>> {
        // Assuming ShoutoutCooldown is now a field in TwitchManager
        self.shoutout_cooldowns.clone()
    }

    pub fn get_user_links(&self) -> Arc<UserLinks> {
        self.user_links.clone()
    }

    pub async fn send_message_as_bot(&self, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.bot_client.send_message(channel, message).await
    }

    pub async fn send_message_as_broadcaster(&self, channel: &str, message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(broadcaster_client) = &self.broadcaster_client {
            broadcaster_client.send_message(channel, message).await
        } else {
            Err("Broadcaster client not initialized".into())
        }
    }

    pub async fn queue_shoutout(&self, user_id: String, username: String) {
        info!("Queueing shoutout for user: {} (ID: {})", username, user_id);
        if let Err(e) = self.shoutout_sender.send((user_id, username)).await {
            error!("Failed to queue shoutout: {:?}", e);
        }
    }

    async fn process_shoutout_queue(&self, mut receiver: mpsc::Receiver<(String, String)>) {
        let cooldown_duration = Duration::from_secs(121); // 2 minutes
        let mut last_processed = Instant::now() - cooldown_duration;

        while let Some((user_id, username)) = receiver.recv().await {
            if last_processed.elapsed() < cooldown_duration {
                info!("Cooldown period not elapsed, delaying API shoutout");
                tokio::time::sleep(cooldown_duration - last_processed.elapsed()).await;
            }

            info!("Processing API shoutout for user: {} (ID: {})", username, user_id);

            if self.is_stream_live().await {
                if let Err(e) = self.execute_api_shoutout(&user_id).await {
                    error!("Failed to execute API shoutout for {}: {:?}", username, e);
                } else {
                    info!("Successfully executed API shoutout for {}", username);
                }
                last_processed = Instant::now();
            } else {
                info!("Stream is offline. Skipping API shoutout for {}.", username);
            }
        }
    }

    async fn execute_api_shoutout(&self, user_id: &str) -> Result<(), Box<dyn StdError + Send + Sync>> {
        info!("Executing API shoutout for user ID: {}", user_id);
        let broadcaster_id = self.api_client.get_broadcaster_id().await?;

        info!("Sending shoutout: broadcaster_id={}, user_id={}, moderator_id={}", broadcaster_id, user_id, broadcaster_id);

        let max_retries = 3;
        for attempt in 1..=max_retries {
            match self.api_client.send_shoutout(&broadcaster_id, user_id, &broadcaster_id).await {
                Ok(_) => {
                    info!("Shoutout sent successfully");
                    return Ok(());
                },
                Err(e) => {
                    error!("Attempt {} failed to send shoutout: {:?}", attempt, e);
                    if attempt == max_retries {
                        return Err(Box::new(e));
                    }
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                }
            }
        }

        Err("Failed to send shoutout after multiple attempts".into())
    }

    // Helper method to convert any error to Box<dyn StdError + Send + Sync>
    fn box_err<E: StdError + Send + Sync + 'static>(e: E) -> Box<dyn StdError + Send + Sync> {
        Box::new(e)
    }

    pub async fn update_streamer_data(&self, user_id: &str) -> Result<(), Box<dyn StdError + Send + Sync>> {
        info!("Updating streamer data for user ID: {}", user_id);
        let api_client = self.get_api_client();

        // Fetch user info
        let user_info = api_client.get_user_info_by_id(user_id).await?;

        let user_data = user_info["data"].as_array()
            .and_then(|arr| arr.first())
            .ok_or_else(|| {
                let err = TwitchAPIError::APIError {
                    status: 404,
                    message: "User data not found".to_string(),
                };
                error!("User data not found in API response for user ID {}", user_id);
                err
            })?;

        let username = user_data["login"].as_str()
            .ok_or_else(|| {
                TwitchAPIError::APIError {
                    status: 500,
                    message: "Username not found in API response".to_string(),
                }
            })?
            .to_string();
        let display_name = user_data["display_name"].as_str().unwrap_or(&username).to_string();

        info!("User info - Username: {}, Display Name: {}", username, display_name);

        // Fetch channel and stream info
        let channel_info = api_client.get_channel_information(user_id).await?;
        let stream_info = api_client.get_stream_info(user_id).await?;
        let top_clips = api_client.get_top_clips(user_id, 10).await?;

        let game_name = channel_info["data"][0]["game_name"].as_str().unwrap_or("").to_string();
        info!("Recent game: {}", game_name);

        let tags = channel_info["data"][0]["tags"].as_array()
            .map(|tags| tags.iter().filter_map(|tag| tag.as_str().map(|s| s.to_string())).collect::<Vec<String>>())
            .unwrap_or_default();
        info!("Current tags: {:?}", tags);

        let current_title = channel_info["data"][0]["title"].as_str().unwrap_or("").to_string();
        info!("Current title: {}", current_title);

        let recent_vods = api_client.get_recent_vods(user_id, 5).await?;

        let mut recent_titles = VecDeque::new();
        recent_titles.push_back(current_title.clone());
        for vod_title in recent_vods {
            recent_titles.push_back(vod_title);
        }
        debug!("Recent titles: {:?}", recent_titles);

        debug!("Top clips: {:?}", top_clips);

        let streamer_data = StreamerData {
            recent_games: vec![game_name],
            current_tags: tags,
            current_title,
            recent_titles,
            top_clips: top_clips.into_iter().map(|clip| (clip.title, clip.url)).collect(),
        };

        // Get the existing user or create a new one
        let mut user = self.user_manager.get_user(user_id).await?;

        // Update the streamer data
        user.streamer_data = Some(streamer_data);

        // Update the user in the UserManager
        self.user_manager.update_user(user_id.to_string(), user).await?;

        info!("Streamer data updated successfully for user_id: {}", user_id);

        Ok(())
    }

    pub async fn start_role_refresh_task(&self) {
        let user_manager = self.user_manager.clone();
        let api_client = self.api_client.clone();

        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(900)).await; // Run every 15 minutes

                let users_to_refresh = {
                    let cache = user_manager.user_cache.read().await;
                    cache.keys().cloned().collect::<Vec<String>>()
                };

                for user_id in users_to_refresh {
                    if let Err(e) = user_manager.get_user(&user_id).await {
                        error!("Error refreshing role for user {}: {:?}", user_id, e);
                    }
                }
            }
        });
    }

    pub async fn preload_broadcaster_data(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let broadcaster_id = self.api_client.get_broadcaster_id().await?;
        debug!("Preloading broadcaster data for ID: {}", broadcaster_id);
        let broadcaster = self.user_manager.get_user(&broadcaster_id).await?;
        debug!("Broadcaster data loaded: {:?}", broadcaster);
        Ok(())
    }
}