use crate::config::Config;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex};
use chrono::{DateTime, Utc, Duration};
use warp::Filter;
use tokio::time::{timeout, sleep, Duration as TokioDuration};
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use log::{debug, error, info, warn};
use serde_json::Value;
use thiserror::Error;
use std::error::Error as StdError;
use crate::twitch::models::channel_points::ChannelPointReward;
use crate::twitch::api::requests::channel::Clip;
use crate::twitch::api::requests::followers;
use crate::twitch::api::requests::followers::FollowerInfo;
use crate::twitch::models::channel_points;

#[derive(Error, Debug)]
pub enum TwitchAPIError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("Failed to parse JSON: {0}")]
    JsonParseError(#[from] serde_json::Error),
    #[error("API returned an error: {status}, {message}")]
    APIError { status: u16, message: String },
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("Generic error: {0}")]
    GenericError(Box<dyn StdError + Send + Sync>),
}

impl From<Box<dyn StdError + Send + Sync>> for TwitchAPIError {
    fn from(error: Box<dyn StdError + Send + Sync>) -> Self {
        TwitchAPIError::GenericError(error)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct TwitchToken {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    #[serde(skip)]
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct TwitchAPIClient {
    config: Arc<Config>,
    token: Arc<Mutex<Option<TwitchToken>>>,
    pub(crate) client: Client,
    initialized: Arc<AtomicBool>,
}

impl Default for TwitchAPIClient {
    fn default() -> Self {
        Self {
            config: Arc::new(Config::default()),
            token: Arc::new(Mutex::new(None)),
            client: Client::new(),
            initialized: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl TwitchAPIClient {
    pub async fn new(config: Arc<Config>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::new();
        let api_client = TwitchAPIClient {
            config: config.clone(),
            token: Arc::new(Mutex::new(None)),
            client,
            initialized: Arc::new(AtomicBool::new(false)),
        };

        // Initialize the token if it exists in the config
        api_client.initialize().await?;

        Ok(api_client)
    }

    async fn authenticated_request<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        query: Option<&[(&str, &str)]>,
        body: Option<serde_json::Value>,
    ) -> Result<T, TwitchAPIError> {
        let token = self.get_token().await?;
        let client_id = self.get_client_id().await?;
        let url = format!("https://api.twitch.tv/helix/{}", endpoint);

        debug!("Sending {} request to {}", method, url);

        let mut request = self.client.request(method, &url)
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token));

        if let Some(q) = query {
            request = request.query(q);
        }

        if let Some(b) = body {
            request = request.json(&b);
        }

        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await?;
            error!("API request failed. Status: {}, Error: {}", status, error_text);
            return Err(TwitchAPIError::APIError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let result: T = response.json().await?;
        debug!("Successfully received and parsed response from {}", url);
        Ok(result)
    }


    pub async fn get_user_info(&self, user_login: &str) -> Result<Value, TwitchAPIError> {
        self.authenticated_request(
            reqwest::Method::GET,
            "users",
            Some(&[("login", user_login)]),
            None,
        ).await
    }

    pub async fn get_user_info_by_id(&self, user_id: &str) -> Result<Value, TwitchAPIError> {
        self.authenticated_request(
            reqwest::Method::GET,
            "users",
            Some(&[("id", user_id)]),
            None,
        ).await
    }

    pub async fn get_stream_info(&self, user_id: &str) -> Result<Value, TwitchAPIError> {
        self.authenticated_request(
            reqwest::Method::GET,
            "streams",
            Some(&[("user_id", user_id)]),
            None,
        ).await
    }

    pub async fn get_channel_information(&self, broadcaster_id: &str) -> Result<Value, TwitchAPIError> {
        self.authenticated_request(
            reqwest::Method::GET,
            "channels",
            Some(&[("broadcaster_id", broadcaster_id)]),
            None,
        ).await
    }

    pub async fn get_top_clips(&self, broadcaster_id: &str, limit: u32) -> Result<Vec<Clip>, Box<dyn std::error::Error + Send + Sync>> {
        let clips: serde_json::Value = self.authenticated_request(
            reqwest::Method::GET,
            "clips",
            Some(&[
                ("broadcaster_id", broadcaster_id),
                ("first", &limit.to_string()),
            ]),
            None,
        ).await?;

        let clips = clips["data"].as_array()
            .ok_or("No clips data found")?
            .iter()
            .map(|clip| Clip {
                title: clip["title"].as_str().unwrap_or("").to_string(),
                url: clip["url"].as_str().unwrap_or("").to_string(),
            })
            .collect();

        Ok(clips)
    }

    pub async fn get_recent_vods(&self, user_id: &str, limit: u32) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let response: serde_json::Value = self.authenticated_request(
            reqwest::Method::GET,
            "videos",
            Some(&[
                ("user_id", user_id),
                ("type", "archive"),
                ("first", &limit.to_string()),
            ]),
            None,
        ).await?;

        let vods = response["data"].as_array()
            .ok_or("No VODs data found")?
            .iter()
            .filter_map(|vod| vod["title"].as_str().map(String::from))
            .collect();

        Ok(vods)
    }

    pub async fn send_shoutout(
        &self,
        from_broadcaster_id: &str,
        to_broadcaster_id: &str,
        moderator_id: &str,
    ) -> Result<(), TwitchAPIError> {
        info!("Sending shoutout from {} to {}", from_broadcaster_id, to_broadcaster_id);
        self.authenticated_request::<serde_json::Value>(
            reqwest::Method::POST,
            "chat/shoutouts",
            None,
            Some(serde_json::json!({
                "from_broadcaster_id": from_broadcaster_id,
                "to_broadcaster_id": to_broadcaster_id,
                "moderator_id": moderator_id,
            })),
        ).await?;

        info!("Shoutout sent successfully");
        Ok(())
    }


    pub async fn authenticate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.config.twitch_access_token.is_some() && self.config.twitch_refresh_token.is_some() {
            println!("Existing Twitch API tokens found. Skipping authentication flow.");
            return Ok(());
        }

        warn!("No existing Twitch API tokens found. Starting authentication flow...");
        self.start_auth_flow().await
    }

    async fn initialize(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let (Some(access_token), Some(refresh_token)) = (&self.config.twitch_access_token, &self.config.twitch_refresh_token) {
            *self.token.lock().await = Some(TwitchToken {
                access_token: access_token.clone(),
                refresh_token: refresh_token.clone(),
                expires_in: 0,
                expires_at: Some(Utc::now()),
            });
            warn!("Existing Twitch API tokens found.");
        } else {
            warn!("No existing Twitch API tokens found. Starting authentication flow...");
            self.start_auth_flow().await?;
        }
        self.initialized.store(true, Ordering::SeqCst);
        info!("Twitch API client fully initialized.");

        Ok(())
    }


    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::SeqCst)
    }

    async fn start_auth_flow(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let auth_code = Arc::new(Mutex::new(None));
        let auth_code_clone = auth_code.clone();

        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        let tx = Arc::new(Mutex::new(Some(tx)));

        let routes = warp::get()
            .and(warp::path("callback"))
            .and(warp::query::<std::collections::HashMap<String, String>>())
            .and(warp::any().map(move || (auth_code_clone.clone(), tx.clone())))
            .and_then(|p: std::collections::HashMap<String, String>,
                       (auth_code, tx): (Arc<Mutex<Option<String>>>, Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>)| async move {
                if let Some(code) = p.get("code") {
                    *auth_code.lock().await = Some(code.to_string());
                    if let Some(tx) = tx.lock().await.take() {
                        let _ = tx.send(());
                    }
                    Ok::<_, Infallible>("Authorization successful! You can close this window now.".to_string())
                } else if let Some(error) = p.get("error") {
                    Ok::<_, Infallible>(format!("Authorization failed: {}. Please try again.", error))
                } else {
                    Ok::<_, Infallible>("Authorization failed. Please try again.".to_string())
                }
            });

        warn!("Starting local server on http://localhost:3000");
        let (addr, server) = warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 3000), async {
            rx.await.ok();
        });

        warn!("Local server running on {}", addr);

        let server_handle = tokio::spawn(server);

        let auth_url = format!(
            "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=http://localhost:3000/callback&response_type=code&scope=chat:read chat:edit channel:read:subscriptions moderator:read:followers moderator:manage:shoutouts channel:read:subscriptions channel:manage:redemptions channel:manage:ads channel:read:ads channel:manage:vips moderation:read moderator:manage:announcements bits:read",
            self.config.twitch_client_id.as_ref().ok_or("Twitch client ID not set")?
        );

        println!("Please open the following URL in your browser to authorize the application:");
        println!("{}", auth_url);

        if webbrowser::open(&auth_url).is_err() {
            println!("Failed to open the browser automatically. Please open the URL manually.");
        }

        let code = match timeout(TokioDuration::from_secs(300), async {
            loop {
                if let Some(code) = auth_code.lock().await.take() {
                    return code;
                }
                sleep(TokioDuration::from_millis(100)).await;
            }
        }).await {
            Ok(code) => code,
            Err(_) => return Err("Timeout waiting for authorization code".into()),
        };

        println!("Received authorization code. Exchanging for token...");

        self.exchange_code(code).await?;

        // Ensure the server is fully shut down
        server_handle.await?;

        println!("Authorization flow completed successfully.");

        Ok(())
    }

    async fn exchange_code(&self, code: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let client_id = self.config.twitch_client_id.as_ref().ok_or("Twitch client ID not set")?;
        let client_secret = self.config.twitch_client_secret.as_ref().ok_or("Twitch client secret not set")?;

        info!("Sending token request...");
        let res = self.client
            .post("https://id.twitch.tv/oauth2/token")
            .form(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("code", &code),
                ("grant_type", &"authorization_code".to_string()),
                ("redirect_uri", &"http://localhost:3000/callback".to_string()),
            ])
            .send()
            .await?;

        let status = res.status();
        info!("Received response with status: {}", status);

        if !status.is_success() {
            let error_text = res.text().await?;
            return Err(format!("Failed to exchange code for token. Status: {}, Error: {}", status, error_text).into());
        }

        let token: TwitchToken = res.json().await?;
        debug!("Successfully parsed token response");
        debug!("Access token (first 10 chars): {}...", &token.access_token[..10]);
        debug!("Refresh token (first 10 chars): {}...", &token.refresh_token[..10]);
        debug!("Token expires in: {} seconds", token.expires_in);

        // Update the token in the TwitchAPIClient
        *self.token.lock().await = Some(token.clone());

        // Update the config with new tokens
        self.update_config_tokens(token.access_token, token.refresh_token).await?;

        debug!("Token exchange and storage completed successfully.");

        Ok(())
    }


    pub async fn get_token(&self) -> Result<String, TwitchAPIError> {
        if !self.is_initialized() {
            self.initialize().await?;
        }

        let token = self.token.lock().await;
        if let Some(token) = &*token {
            if token.expires_at.map_or(false, |expires_at| expires_at > Utc::now()) {
                return Ok(token.access_token.clone());
            }
        }
        drop(token);

        self.refresh_token().await
    }

    pub(crate) async fn refresh_token(&self) -> Result<String, TwitchAPIError> {
        let token = self.token.lock().await;
        let refresh_token = token.as_ref()
            .map(|t| t.refresh_token.clone())
            .ok_or_else(|| TwitchAPIError::AuthError("No refresh token available".to_string()))?;
        drop(token);

        let res = self.client
            .post("https://id.twitch.tv/oauth2/token")
            .form(&[
                ("client_id", self.config.twitch_client_id.as_ref().ok_or_else(|| TwitchAPIError::ConfigError("Twitch API client ID not set".to_string()))?),
                ("client_secret", self.config.twitch_client_secret.as_ref().ok_or_else(|| TwitchAPIError::ConfigError("Twitch API client secret not set".to_string()))?),
                ("refresh_token", &refresh_token),
                ("grant_type", &"refresh_token".to_string()),
            ])
            .send()
            .await?
            .json::<TwitchToken>()
            .await?;

        let mut new_token = res;
        new_token.expires_at = Some(Utc::now() + Duration::seconds(new_token.expires_in));

        let access_token = new_token.access_token.clone();
        *self.token.lock().await = Some(new_token.clone());

        // Update the config with new tokens
        self.update_config_tokens(new_token.access_token, new_token.refresh_token).await?;

        Ok(access_token)
    }

    async fn update_config_tokens(&self, access_token: String, refresh_token: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut config = self.config.as_ref().clone();
        config.twitch_access_token = Some(access_token);
        config.twitch_refresh_token = Some(refresh_token);
        config.save()?;
        Ok(())
    }

    pub async fn is_stream_live(&self, user_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let stream_info = self.get_stream_info(user_id).await?;
        Ok(!stream_info["data"].as_array().unwrap_or(&vec![]).is_empty())
    }

    pub async fn get_client_id(&self) -> Result<String, TwitchAPIError> {
        let config = self.config.as_ref();
        config.twitch_client_id.clone().ok_or_else(|| TwitchAPIError::ConfigError("Twitch client ID not set".to_string()))
    }

    pub async fn get_broadcaster_id(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.as_ref();
        let channel_name = config.twitch_channel_to_join.clone().ok_or("Channel name not set")?;
        let _ = config;

        let user_info = self.get_user_info(&channel_name).await?;
        let channel_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get channel ID")?.to_string();

        Ok(channel_id)
    }

    pub async fn get_bot_id(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.as_ref();
        let bot_name = config.twitch_bot_username.clone().ok_or("Bot name not set")?;
        let _ = config;
        info!("bot username: {}", bot_name);
        let user_info = self.get_user_info(&bot_name).await?;
        let bot_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get bot ID")?.to_string();

        Ok(bot_id)
    }

    pub async fn get_follower_count(&self, broadcaster_id: &str) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        followers::get_follower_count(self, broadcaster_id).await
    }

    pub async fn get_follower_info(&self, broadcaster_id: &str, user_id: Option<&str>) -> Result<(Vec<FollowerInfo>, u32), Box<dyn std::error::Error + Send + Sync>> {
        followers::get_follower_info(self, broadcaster_id, user_id).await
    }

    pub async fn update_redemption_status(
        &self,
        reward_id: &str,
        redemption_id: &str,
        status: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let broadcaster_id = self.get_broadcaster_id().await?;
        crate::twitch::api::requests::channel_points::update_redemption_status(
            self,
            &broadcaster_id,
            reward_id,
            redemption_id,
            status,
        ).await
    }

    pub async fn refund_channel_points(
        &self,
        reward_id: &str,
        redemption_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let broadcaster_id = self.get_broadcaster_id().await?;
        crate::twitch::api::requests::channel_points::refund_channel_points(
            self,
            &broadcaster_id,
            reward_id,
            redemption_id,
        ).await
    }

    pub async fn complete_channel_points(
        &self,
        broadcaster_id: &str,
        reward_id: &str,
        redemption_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::twitch::api::requests::channel_points::complete_channel_points(
            self,
            broadcaster_id,
            reward_id,
            redemption_id,
        ).await
    }

    pub async fn get_custom_reward(&self, reward_id: &str) -> Result<ChannelPointReward, Box<dyn std::error::Error + Send + Sync>> {
        let broadcaster_id = self.get_broadcaster_id().await?;
        let response = self.client
            .get("https://api.twitch.tv/helix/channel_points/custom_rewards")
            .header("Client-ID", self.get_client_id().await?)
            .header("Authorization", format!("Bearer {}", self.get_token().await?))
            .query(&[
                ("broadcaster_id", broadcaster_id.as_str()),
                ("id", reward_id),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to get custom reward. Status: {}", response.status()).into());
        }

        let body: serde_json::Value = response.json().await?;
        let reward = &body["data"][0];

        Ok(ChannelPointReward {
            id: reward["id"].as_str().unwrap_or("").to_string(),
            title: reward["title"].as_str().unwrap_or("").to_string(),
            cost: reward["cost"].as_u64().unwrap_or(0) as u32,
            is_enabled: reward["is_enabled"].as_bool().unwrap_or(false),
            is_in_stock: reward["is_in_stock"].as_bool().unwrap_or(false),
            is_paused: reward["is_paused"].as_bool().unwrap_or(false),
            is_user_input_required: reward["is_user_input_required"].as_bool().unwrap_or(false),
            prompt: reward["prompt"].as_str().unwrap_or("").to_string(),
            cooldown_seconds: reward["cooldown_seconds"].as_u64().map(|v| v as u32),
            global_cooldown: reward["global_cooldown"].as_object().map(|gc| channel_points::GlobalCooldown {
                is_enabled: gc["is_enabled"].as_bool().unwrap_or(false),
                global_cooldown_seconds: gc["global_cooldown_seconds"].as_u64().unwrap_or(0) as u32,
            }),
            max_per_stream: reward["max_per_stream"].as_object().map(|mps| channel_points::MaxPerStream {
                is_enabled: mps["is_enabled"].as_bool().unwrap_or(false),
                max_per_stream: mps["max_per_stream"].as_u64().unwrap_or(0) as u32,
            }),
            max_per_user_per_stream: reward["max_per_user_per_stream"].as_object().map(|mpups| channel_points::MaxPerUserPerStream {
                is_enabled: mpups["is_enabled"].as_bool().unwrap_or(false),
                max_per_user_per_stream: mpups["max_per_user_per_stream"].as_u64().unwrap_or(0) as u32,
            }),
        })
    }


    pub async fn get_channel_point_rewards(&self) -> Result<Vec<ChannelPointReward>, Box<dyn std::error::Error + Send + Sync>> {
        let token = self.get_token().await?;
        let client_id = self.get_client_id().await?;
        let broadcaster_id = self.get_broadcaster_id().await?;

        let response = self.client
            .get("https://api.twitch.tv/helix/channel_points/custom_rewards")
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .query(&[("broadcaster_id", broadcaster_id)])
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(format!("Failed to get channel point rewards. Status: {}, Error: {}", status, error_text).into());
        }

        let body: serde_json::Value = response.json().await?;
        let rewards = body["data"].as_array()
            .ok_or("No rewards data found")?
            .iter()
            .map(|reward| ChannelPointReward {
                id: reward["id"].as_str().unwrap_or("").to_string(),
                title: reward["title"].as_str().unwrap_or("").to_string(),
                cost: reward["cost"].as_u64().unwrap_or(0) as u32,
                is_enabled: reward["is_enabled"].as_bool().unwrap_or(false),
                is_in_stock: reward["is_in_stock"].as_bool().unwrap_or(false),
                is_paused: reward["is_paused"].as_bool().unwrap_or(false),
                is_user_input_required: reward["is_user_input_required"].as_bool().unwrap_or(false),
                prompt: reward["prompt"].as_str().unwrap_or("").to_string(),
                cooldown_seconds: reward["cooldown_seconds"].as_u64().map(|v| v as u32),
                global_cooldown: reward["global_cooldown"].as_object().map(|gc| channel_points::GlobalCooldown {
                    is_enabled: gc["is_enabled"].as_bool().unwrap_or(false),
                    global_cooldown_seconds: gc["global_cooldown_seconds"].as_u64().unwrap_or(0) as u32,
                }),
                max_per_stream: reward["max_per_stream"].as_object().map(|mps| channel_points::MaxPerStream {
                    is_enabled: mps["is_enabled"].as_bool().unwrap_or(false),
                    max_per_stream: mps["max_per_stream"].as_u64().unwrap_or(0) as u32,
                }),
                max_per_user_per_stream: reward["max_per_user_per_stream"].as_object().map(|mpups| channel_points::MaxPerUserPerStream {
                    is_enabled: mpups["is_enabled"].as_bool().unwrap_or(false),
                    max_per_user_per_stream: mpups["max_per_user_per_stream"].as_u64().unwrap_or(0) as u32,
                }),
            })
            .collect();

        Ok(rewards)
    }

    pub async fn is_user_moderator(&self, user_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let token = self.get_token().await?;
        let client_id = self.get_client_id().await?;
        let broadcaster_id = self.get_broadcaster_id().await?;

        let response = self.client
            .get("https://api.twitch.tv/helix/moderation/moderators")
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .query(&[
                ("broadcaster_id", broadcaster_id.as_str()),
                ("user_id", user_id),
            ])
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(format!("Failed to check moderator status. Status: {}, Error: {}", status, error_text).into());
        }

        let body: serde_json::Value = response.json().await?;
        Ok(!body["data"].as_array().map_or(true, |arr| arr.is_empty()))
    }

    pub async fn create_custom_reward(
        &self,
        title: &str,
        cost: u32,
        is_enabled: bool,
        cooldown: u32,
        prompt: &str,
        user_input_required: bool,
    ) -> Result<ChannelPointReward, Box<dyn std::error::Error + Send + Sync>> {
        let token = self.get_token().await?;
        let client_id = self.get_client_id().await?;
        let broadcaster_id = self.get_broadcaster_id().await?;

        let url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}",
            broadcaster_id
        );

        let body = serde_json::json!({
            "title": title,
            "cost": cost,
            "is_enabled": is_enabled,
            "is_global_cooldown_enabled": cooldown > 0,
            "global_cooldown_seconds": cooldown,
            "prompt": prompt,
            "is_user_input_required": user_input_required,  // Add this field
        });

        debug!("Creating custom reward. URL: {}, Body: {:?}", url, body);

        let response = self.client
            .post(&url)
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            let response_body: serde_json::Value = response.json().await?;
            let reward: ChannelPointReward = serde_json::from_value(response_body["data"][0].clone())?;
            Ok(reward)
        } else {
            let error_body = response.text().await?;
            Err(format!("Failed to create custom reward. Status: {}, Body: {}", status, error_body).into())
        }
    }

    pub async fn delete_custom_reward(&self, broadcaster_id: &str, reward_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::twitch::api::requests::channel_points::delete_custom_reward(self, broadcaster_id, reward_id).await
    }

    pub async fn update_custom_reward(
        &self,
        reward_id: &str,
        title: &str,
        cost: u32,
        is_enabled: bool,
        cooldown: u32,
        prompt: &str,
        user_input_required: bool,
    ) -> Result<ChannelPointReward, Box<dyn std::error::Error + Send + Sync>> {
        let token = self.get_token().await?;
        let client_id = self.get_client_id().await?;
        let broadcaster_id = self.get_broadcaster_id().await?;

        let url = format!(
            "https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}&id={}",
            broadcaster_id, reward_id
        );

        let body = serde_json::json!({
            "title": title,
            "cost": cost,
            "is_enabled": is_enabled,
            "is_global_cooldown_enabled": cooldown > 0,
            "global_cooldown_seconds": cooldown,
            "prompt": prompt,
            "is_user_input_required": user_input_required,  // Add this field
        });

        debug!("Updating custom reward. URL: {}, Body: {:?}", url, body);

        let response = self.client
            .patch(&url)
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            let response_body: serde_json::Value = response.json().await?;
            let reward: ChannelPointReward = serde_json::from_value(response_body["data"][0].clone())?;
            Ok(reward)
        } else {
            let error_body = response.text().await?;
            Err(format!("Failed to update custom reward. Status: {}, Body: {}", status, error_body).into())
        }
    }
}

impl TwitchAPIClient {
    pub async fn check_user_mod(&self, broadcaster_id: &str, user_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://api.twitch.tv/helix/moderation/moderators?broadcaster_id={}&user_id={}",
            broadcaster_id, user_id
        );
        let response = self.send_authenticated_request(&url).await?;
        let data: serde_json::Value = response.json().await?;
        Ok(!data["data"].as_array().unwrap_or(&vec![]).is_empty())
    }

    pub async fn check_user_vip(&self, broadcaster_id: &str, user_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://api.twitch.tv/helix/channels/vips?broadcaster_id={}&user_id={}",
            broadcaster_id, user_id
        );
        let response = self.send_authenticated_request(&url).await?;
        let data: serde_json::Value = response.json().await?;
        Ok(!data["data"].as_array().unwrap_or(&vec![]).is_empty())
    }

    pub async fn check_user_subscription(&self, broadcaster_id: &str, user_id: &str) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "https://api.twitch.tv/helix/subscriptions?broadcaster_id={}&user_id={}",
            broadcaster_id, user_id
        );
        let response = self.send_authenticated_request(&url).await?;
        let data: serde_json::Value = response.json().await?;
        Ok(!data["data"].as_array().unwrap_or(&vec![]).is_empty())
    }

    async fn send_authenticated_request(&self, url: &str) -> Result<reqwest::Response, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.as_ref();
        let access_token = config.twitch_access_token.clone().ok_or("Twitch access token not set")?;
        let client_id = config.twitch_client_id.clone().ok_or("Twitch client ID not set")?;
        let _ = config;

        let client = reqwest::Client::new();
        let response = client.get(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Client-Id", client_id)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await?;
            println!("API request failed. Status: {}, URL: {}", status, url);
            println!("Response body: {:?}", error_body);
            return Err(format!("API request failed. Status: {}, Body: {}", status, error_body).into());
        }

        Ok(response)
    }
}