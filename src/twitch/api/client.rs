use crate::config::Config;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use std::time::{Duration, Instant};
use warp::Filter;
use tokio::time::timeout;
use std::convert::Infallible;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::twitch::api::models::ChannelPointReward;

#[derive(Debug, Serialize, Deserialize)]
struct TwitchToken {
    access_token: String,
    refresh_token: String,
    expires_in: u64,
    #[serde(skip)]
    expires_at: Option<Instant>,
}

#[derive(Clone)]
pub struct TwitchAPIClient {
    config: Arc<RwLock<Config>>,
    token: Arc<RwLock<Option<TwitchToken>>>,
    pub(crate) client: Client,
    initialized: Arc<AtomicBool>,
}

impl TwitchAPIClient {
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let client = Client::new();
        let api_client = TwitchAPIClient {
            config: config.clone(),
            token: Arc::new(RwLock::new(None)),
            client,
            initialized: Arc::new(AtomicBool::new(false)),
        };

        // Initialize the token if it exists in the config
        api_client.initialize().await?;

        Ok(api_client)
    }

    pub async fn authenticate(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.read().await;
        if config.twitch_access_token.is_some() && config.twitch_refresh_token.is_some() {
            println!("Existing Twitch API tokens found. Skipping authentication flow.");
            return Ok(());
        }
        drop(config);

        println!("No existing Twitch API tokens found. Starting authentication flow...");
        self.start_auth_flow().await
    }

    async fn initialize(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.read().await;
        if let (Some(access_token), Some(refresh_token)) = (&config.twitch_access_token, &config.twitch_refresh_token) {
            *self.token.write().await = Some(TwitchToken {
                access_token: access_token.clone(),
                refresh_token: refresh_token.clone(),
                expires_in: 0,
                expires_at: Some(Instant::now()),
            });
            println!("Existing Twitch API tokens found.");
        } else {
            drop(config);
            println!("No existing Twitch API tokens found. Starting authentication flow...");
            self.start_auth_flow().await?;
        }
        self.initialized.store(true, Ordering::SeqCst);
        println!("Twitch API client fully initialized.");

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

        println!("Starting local server on http://localhost:3000");
        let (addr, server) = warp::serve(routes).bind_with_graceful_shutdown(([127, 0, 0, 1], 3000), async {
            rx.await.ok();
        });

        println!("Local server running on {}", addr);

        let server_handle = tokio::spawn(server);

        let config = self.config.read().await;
        let auth_url = format!(
            "https://id.twitch.tv/oauth2/authorize?client_id={}&redirect_uri=http://localhost:3000/callback&response_type=code&scope=chat:read chat:edit channel:read:subscriptions moderator:read:followers moderator:manage:shoutouts channel:read:subscriptions channel:manage:redemptions",
            config.twitch_client_id.as_ref().ok_or("Twitch client ID not set")?
        );
        drop(config);

        println!("Please open the following URL in your browser to authorize the application:");
        println!("{}", auth_url);

        if webbrowser::open(&auth_url).is_err() {
            println!("Failed to open the browser automatically. Please open the URL manually.");
        }

        let code = match timeout(Duration::from_secs(300), async {
            loop {
                if let Some(code) = auth_code.lock().await.take() {
                    return code;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
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
        let config_read = self.config.read().await;
        let client_id = config_read.twitch_client_id.as_ref().ok_or("Twitch client ID not set")?;
        let client_secret = config_read.twitch_client_secret.as_ref().ok_or("Twitch client secret not set")?;

        println!("Sending token request...");
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
        println!("Received response with status: {}", status);

        if !status.is_success() {
            let error_text = res.text().await?;
            return Err(format!("Failed to exchange code for token. Status: {}, Error: {}", status, error_text).into());
        }

        let token: TwitchToken = res.json().await?;
        println!("Successfully parsed token response");
        println!("Access token (first 10 chars): {}...", &token.access_token[..10]);
        println!("Refresh token (first 10 chars): {}...", &token.refresh_token[..10]);
        println!("Token expires in: {} seconds", token.expires_in);

        // Drop the read lock before acquiring the write lock
        drop(config_read);

        // Now, update the config with the new tokens
        let mut config_write = self.config.write().await;
        config_write.set_twitch_tokens(token.access_token.clone(), token.refresh_token.clone())?;
        println!("Tokens saved to config file");

        // Update the token in the TwitchAPIClient
        *self.token.write().await = Some(token);

        println!("Token exchange and storage completed successfully.");

        Ok(())
    }

    pub async fn get_token(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if !self.is_initialized() {
            self.initialize().await?;
        }

        let token = self.token.read().await;
        if let Some(token) = &*token {
            if token.expires_at.map_or(false, |expires_at| expires_at > Instant::now()) {
                return Ok(token.access_token.clone());
            }
        }
        drop(token);

        self.refresh_token().await
    }

    pub(crate) async fn refresh_token(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let token = self.token.read().await;
        let refresh_token = token.as_ref().map(|t| t.refresh_token.clone()).ok_or("No refresh token available")?;
        drop(token);

        let config = self.config.read().await;
        let res = self.client
            .post("https://id.twitch.tv/oauth2/token")
            .form(&[
                ("client_id", config.twitch_client_id.as_ref().ok_or("Twitch API client ID not set")?),
                ("client_secret", config.twitch_client_secret.as_ref().ok_or("Twitch API client secret not set")?),
                ("refresh_token", &refresh_token),
                ("grant_type", &"refresh_token".to_string()),
            ])
            .send()
            .await?
            .json::<TwitchToken>()
            .await?;

        let mut new_token = res;
        new_token.expires_at = Some(Instant::now() + Duration::from_secs(new_token.expires_in));

        drop(config);

        let mut config = self.config.write().await;
        config.twitch_access_token = Some(new_token.access_token.clone());
        config.twitch_refresh_token = Some(new_token.refresh_token.clone());
        config.save()?;

        let access_token = new_token.access_token.clone();
        *self.token.write().await = Some(new_token);

        Ok(access_token)
    }

    // Add methods for making API calls here, e.g.:
    pub async fn get_user_info(&self, user_login: &str) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let token = self.get_token().await?;
        let config = self.config.read().await;
        let client_id = config.twitch_client_id.as_ref().ok_or("Twitch API client ID not set")?;

        println!("Sending request to Twitch API for user info: {}", user_login);

        let response = self.client
            .get(&format!("https://api.twitch.tv/helix/users?login={}", user_login))
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        println!("Received response from Twitch API. Status: {}", response.status());

        let body = response.text().await?;
        println!("Response body: {}", body);

        let json: serde_json::Value = serde_json::from_str(&body)?;

        if json["data"].as_array().map_or(true, |arr| arr.is_empty()) {
            return Err(format!("User not found: {}", user_login).into());
        }

        Ok(json)
    }

    pub async fn get_client_id(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.read().await;
        config.twitch_client_id.clone().ok_or_else(|| "Twitch client ID not set".into())
    }

    pub async fn get_broadcaster_id(&self) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let config = self.config.read().await;
        let channel_name = config.twitch_channel_to_join.clone().ok_or("Channel name not set")?;
        drop(config);

        let user_info = self.get_user_info(&channel_name).await?;
        let channel_id = user_info["data"][0]["id"].as_str().ok_or("Failed to get channel ID")?.to_string();

        Ok(channel_id)
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
        let rewards: Vec<ChannelPointReward> = serde_json::from_value(body["data"].clone())?;

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
        is_user_input_required: bool,
    ) -> Result<ChannelPointReward, Box<dyn std::error::Error + Send + Sync>> {
        let token = self.get_token().await?;
        let client_id = self.get_client_id().await?;
        let broadcaster_id = self.get_broadcaster_id().await?;

        let response = self.client
            .post(&format!("https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}", broadcaster_id))
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({
            "title": title,
            "cost": cost,
            "is_user_input_required": is_user_input_required,
            "is_enabled": true
        }))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(format!("Failed to create custom reward. Status: {}, Error: {}", status, error_text).into());
        }

        let body: serde_json::Value = response.json().await?;
        let reward: ChannelPointReward = serde_json::from_value(body["data"][0].clone())?;

        Ok(reward)
    }

    pub async fn update_custom_reward(
        &self,
        reward_id: &str,
        title: &str,
        cost: u32,
        is_enabled: bool,
    ) -> Result<ChannelPointReward, Box<dyn std::error::Error + Send + Sync>> {
        let token = self.get_token().await?;
        let client_id = self.get_client_id().await?;
        let broadcaster_id = self.get_broadcaster_id().await?;

        let response = self.client
            .patch(&format!("https://api.twitch.tv/helix/channel_points/custom_rewards?broadcaster_id={}&id={}", broadcaster_id, reward_id))
            .header("Client-ID", client_id)
            .header("Authorization", format!("Bearer {}", token))
            .json(&serde_json::json!({
            "title": title,
            "cost": cost,
            "is_enabled": is_enabled
        }))
            .send()
            .await?;

        let body: serde_json::Value = response.json().await?;
        let reward: ChannelPointReward = serde_json::from_value(body["data"][0].clone())?;

        Ok(reward)
    }
}