use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::sync::Arc;
use chrono::{DateTime, Utc};
use log::{error, info};
use tokio::sync::{mpsc, Mutex, RwLock};
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

#[derive(Clone)]
pub struct TwitchUser {
    user_id: String,
    username: String,
    display_name: String,
    pub(crate) role: UserRole,
    last_seen: DateTime<Utc>,
    messages: VecDeque<(DateTime<Utc>, String)>,
    streamer_data: Option<StreamerData>,
}

#[derive(Clone)]
pub struct StreamerData {
    recent_games: Vec<String>,
    current_tags: Vec<String>,
    current_title: String,
    recent_titles: VecDeque<String>,
    top_clips: Vec<(String, String)>, // (clip_title, clip_url)
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

    pub async fn get_user(&self, user_id: &str) -> Result<TwitchUser, Box<dyn Error + Send + Sync>> {
        // Check cache
        if let Some(user) = self.user_cache.read().await.get(user_id) {
            return Ok(user.clone());
        }

        // Check storage
        let storage_read = self.storage.read().await;
        if let Some(chatter_data) = storage_read.get_chatter_data(user_id)? {
            let user = TwitchUser {
                user_id: chatter_data.user_id,
                username: chatter_data.username,
                display_name: "displayname".to_string(),
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
        let user = TwitchUser {
            user_id: user_id.to_string(),
            username: user_info["data"][0]["login"].as_str().unwrap().to_string(),
            display_name: user_info["data"][0]["display_name"].as_str().unwrap().to_string(),
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
        ));

        let (bot_client, broadcaster_client) = Self::initialize_irc_clients(&config, &irc_manager).await?;

        let redeem_manager = Arc::new(RwLock::new(RedeemManager::new(
            ai_client.clone(),
            vrchat_osc.clone(),
            api_client.clone(),
        )));

        let user_manager = UserManager::new(storage.clone(), api_client.clone());

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

        Ok(twitch_manager)
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement shutdown logic here
        // For example:
        // - Stop any running tasks
        // - Close connections
        // - Save any necessary state
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

        let broadcaster_client = if let Some(broadcaster_oauth_token) = &config.twitch_access_token {
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
        self.redeem_manager.write().await.set_stream_live(is_live).await.unwrap_or_else(|e| {
            error!("Failed to set stream status in redeem manager: {}", e);
        });
    }

    pub async fn get_user(&self, user_id: &str) -> Result<TwitchUser, Box<dyn std::error::Error + Send + Sync>> {
        self.user_manager.get_user(user_id).await
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

    pub fn get_api_client(&self) -> Arc<TwitchAPIClient> {
        self.api_client.clone()
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

    pub async fn update_streamer_data(&self, user_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut user = self.user_manager.get_user(user_id).await?;

        // Fetch streamer data from Twitch API
        let channel_info = crate::twitch::api::requests::get_channel_information(&self.api_client, user_id).await?;
        let stream_info = self.api_client.get_stream_info(user_id).await?;
        let top_clips = crate::twitch::api::requests::get_top_clips(&self.api_client, user_id, 10).await?;

        user.streamer_data = Some(StreamerData {
            recent_games: vec![channel_info["data"][0]["game_name"].as_str().unwrap_or("").to_string()],
            current_tags: channel_info["data"][0]["tags"].as_array()
                .map(|tags| tags.iter().filter_map(|tag| tag.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default(),
            current_title: channel_info["data"][0]["title"].as_str().unwrap_or("").to_string(),
            recent_titles: {
                let mut titles = VecDeque::new();
                titles.push_back(channel_info["data"][0]["title"].as_str().unwrap_or("").to_string());
                titles
            },
            top_clips: top_clips.into_iter().map(|clip| (clip.title, clip.url)).collect(),
        });

        // Update user in the user manager
        self.user_manager.update_user(user_id.to_string(), user).await?;

        Ok(())
    }
}