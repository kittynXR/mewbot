use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::error::Error as StdError;
use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::{DateTime, Utc};
use futures_util::TryFutureExt;
use log::{error, info, warn};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio::time::timeout;
use crate::ai::AIClient;
use crate::config::Config;
use crate::discord::UserLinks;
use crate::osc::osc_config::OSCConfigurations;
use crate::osc::VRChatOSC;
use crate::storage::{ChatterData, StorageClient};
use crate::twitch::{TwitchAPIClient, TwitchIRCManager};
use crate::twitch::eventsub::TwitchEventSubClient;
use crate::twitch::irc::{MessageHandler, TwitchBotClient, TwitchBroadcasterClient};
use crate::twitch::redeems::RedeemManager;
use crate::twitch::roles::UserRole;
use crate::web_ui::websocket::{DashboardState, WebSocketMessage};
use crate::twitch::irc::commands::ad_commands::AdManager;

#[derive(Clone)]
pub struct TwitchUser {
    user_id: String,
    pub(crate) username: String,
    pub(crate) display_name: String,
    pub(crate) role: UserRole,
    last_seen: DateTime<Utc>,
    messages: VecDeque<(DateTime<Utc>, String)>,
    pub(crate) streamer_data: Option<StreamerData>,
}

#[derive(Clone)]
pub struct StreamerData {
    pub(crate) recent_games: Vec<String>,
    pub(crate) current_tags: Vec<String>,
    pub(crate) current_title: String,
    recent_titles: VecDeque<String>,
    pub(crate) top_clips: Vec<(String, String)>, // (clip_title, clip_url)
}

#[derive(Clone)]
pub struct UserManager {
    user_cache: Arc<RwLock<HashMap<String, TwitchUser>>>,
    storage: Arc<RwLock<StorageClient>>,
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
    pub fn new(storage: Arc<RwLock<StorageClient>>, api_client: Arc<TwitchAPIClient>) -> Self {
        Self {
            user_cache: Arc::new(RwLock::new(HashMap::new())),
            storage,
            api_client,
        }
    }

    pub async fn get_user(&self, user_id: &str) -> Result<TwitchUser, Box<dyn std::error::Error + Send + Sync>> {
        // Check cache
        if let Some(user) = self.user_cache.read().await.get(user_id) {
            return Ok(user.clone());
        }

        // Check storage
        let storage_read = self.storage.read().await;
        if let Some(chatter_data) = storage_read.get_chatter_data(user_id)? {
            let username = chatter_data.username.clone(); // Clone here
            let user = TwitchUser {
                user_id: chatter_data.user_id,
                username: username.clone(), // Use the cloned value
                display_name: username, // Use the cloned value as display_name
                role: chatter_data.role,
                last_seen: chatter_data.last_seen,
                messages: VecDeque::new(),
                streamer_data: None,
            };
            drop(storage_read);
            self.user_cache.write().await.insert(user_id.to_string(), user.clone());
            return Ok(user);
        }
        drop(storage_read);

        // Fetch from API
        let user_info = self.api_client.get_user_info(user_id).await?;
        let user_data = user_info["data"].as_array()
            .and_then(|arr| arr.first())
            .ok_or_else(|| Box::new(std::io::Error::new(std::io::ErrorKind::NotFound, "User data not found")))?;

        let username = user_data["login"].as_str()
            .ok_or_else(|| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, "Username not found")))?
            .to_string();
        let display_name = user_data["display_name"].as_str().unwrap_or(&username).to_string();

        let user = TwitchUser {
            user_id: user_id.to_string(),
            username,
            display_name,
            role: UserRole::Viewer,
            last_seen: Utc::now(),
            messages: VecDeque::new(),
            streamer_data: None,
        };

        // Update cache and storage
        self.user_cache.write().await.insert(user_id.to_string(), user.clone());
        let mut storage_write = self.storage.write().await;
        storage_write.upsert_chatter(&user.clone().into())?;

        Ok(user)
    }

    pub async fn update_user(&self, user_id: String, user: TwitchUser) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.user_cache.write().await.insert(user_id.clone(), user.clone());
        let mut storage_write = self.storage.write().await;
        storage_write.upsert_chatter(&user.into())?;
        Ok(())
    }

    pub async fn update_user_role(&self, user_id: &str, role: UserRole) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut user = self.get_user(user_id).await?;
        user.role = role;

        // Update cache
        self.user_cache.write().await.insert(user_id.to_string(), user.clone());

        // Update storage
        let mut storage_write = self.storage.write().await;
        storage_write.upsert_chatter(&user.into())?;

        Ok(())
    }

    pub async fn add_user_message(&self, user_id: &str, message: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut user = self.get_user(user_id).await?;
        user.messages.push_front((Utc::now(), message));
        if user.messages.len() > 100 {
            user.messages.pop_back();
        }

        // Update cache
        self.user_cache.write().await.insert(user_id.to_string(), user);

        Ok(())
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

        let redeem_manager = Arc::new(RwLock::new(RedeemManager::new(
            api_client.clone(),
            ai_client.clone().unwrap_or_else(|| Arc::new(AIClient::new(None, None))),
            vrchat_osc.clone().unwrap_or_else(|| Arc::new(VRChatOSC::new("127.0.0.1:9000").expect("Failed to create VRChatOSC"))),
            osc_configs.clone(),
        )));

        let user_manager = UserManager::new(storage.clone(), api_client.clone());

        let shoutout_queue = Arc::new(Mutex::new(VecDeque::new()));
        let shoutout_last_processed = Arc::new(Mutex::new(Instant::now() - Duration::from_secs(120)));

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
        let tm_clone = Arc::new(twitch_manager.clone());
        tokio::spawn(async move {
            tm_clone.process_shoutout_queue().await;
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

    async fn process_shoutout_queue(&self) {
        let cooldown_duration = Duration::from_secs(120); // 2 minutes

        loop {
            tokio::time::sleep(Duration::from_secs(5)).await; // Check queue every 5 seconds

            let mut last_processed = self.shoutout_last_processed.lock().await;
            if last_processed.elapsed() < cooldown_duration {
                continue;
            }

            let mut queue = self.shoutout_queue.lock().await;
            if let Some((user_id, username)) = queue.pop_front() {
                // Process the API shoutout
                if let Err(e) = self.execute_api_shoutout(&user_id).await {
                    error!("Failed to execute API shoutout for {}: {:?}", username, e);
                    // Optionally, we could push the failed shoutout back to the queue
                }
                *last_processed = Instant::now();
            }
        }
    }

    async fn execute_api_shoutout(&self, user_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let broadcaster_id = self.api_client.get_broadcaster_id().await?;
        let moderator_id = self.api_client.get_bot_id().await?;

        // Send the API shoutout
        self.api_client.send_shoutout(&broadcaster_id, user_id, &moderator_id).await?;

        Ok(())
    }

    pub async fn queue_shoutout(&self, user_id: String, username: String) {
        let mut queue = self.shoutout_queue.lock().await;
        queue.push_back((user_id, username));
    }

    pub async fn update_streamer_data(&self, user_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let api_client = self.get_api_client();

        // Fetch user info
        let user_info = api_client.get_user_info_by_id(user_id).await?;
        let user_data = user_info["data"].as_array()
            .and_then(|arr| arr.first())
            .ok_or("User data not found")?;

        let username = user_data["login"].as_str().ok_or("Failed to get username")?.to_string();
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

        let mut recent_titles = VecDeque::new();
        recent_titles.push_back(current_title.clone());
        info!("Recent titles: {:?}", recent_titles);

        info!("Top clips:");
        for clip in &top_clips {
            info!("  Title: {}, URL: {}", clip.title, clip.url);
        }

        let streamer_data = StreamerData {
            recent_games: vec![game_name],
            current_tags: tags,
            current_title,
            recent_titles,
            top_clips: top_clips.into_iter().map(|clip| (clip.title, clip.url)).collect(),
        };

        // Update user in the user manager
        let user = TwitchUser {
            user_id: user_id.to_string(),
            username,
            display_name,
            role: UserRole::Viewer, // You might want to fetch the actual role if available
            last_seen: chrono::Utc::now(),
            messages: VecDeque::new(),
            streamer_data: Some(streamer_data),
        };

        self.user_manager.update_user(user_id.to_string(), user).await?;

        info!("Streamer data updated successfully for user_id: {}", user_id);

        Ok(())
    }
}