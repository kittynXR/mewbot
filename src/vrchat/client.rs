use crate::vrchat::models::{Friend, VRChatError, VRChatStatus};
use crate::config::Config;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, COOKIE};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::sync::Mutex;
use std::io::{self, Write};
use log::{info, warn};
use rpassword::read_password;
use tokio::sync::mpsc;
use crate::vrchat::World;
use crate::web_ui::websocket::WebSocketMessage;

pub struct VRChatClient {
    client: Client,
    auth_cookie: Arc<RwLock<String>>,
    config: Arc<RwLock<Config>>,
    current_user_id: Arc<RwLock<Option<String>>>,
    current_world: Arc<RwLock<Option<World>>>,
    websocket_tx: mpsc::UnboundedSender<WebSocketMessage>,
}

impl VRChatClient {
    pub async fn new(config: Arc<RwLock<Config>>, websocket_tx: mpsc::UnboundedSender<WebSocketMessage>) -> Result<Self, VRChatError> {
        let client = Client::builder()
            .user_agent("kittynvrc/twitchbot")
            .build()
            .map_err(|e| VRChatError(format!("Failed to build client: {}", e)))?;

        let auth_cookie = {
            let config_read = config.read().await;
            match &config_read.vrchat_auth_cookie {
                Some(cookie) => cookie.clone(),
                None => {
                    drop(config_read);
                    println!("VRChat auth cookie not found. Please log in.");
                    let cookie = Self::login(&client).await?;
                    let mut config_write = config.write().await;
                    config_write.set_vrchat_auth_cookie(cookie.clone())?;
                    cookie
                }
            }
        };

        Ok(VRChatClient {
            client,
            auth_cookie: Arc::new(RwLock::new(auth_cookie)),
            config,
            current_user_id: Arc::new(RwLock::new(None)),
            current_world: Arc::new(RwLock::new(None)),
            websocket_tx,
        })
    }

    pub async fn is_online(&self) -> bool {
        self.current_user_id.read().await.is_some()
    }

    async fn login(client: &Client) -> Result<String, VRChatError> {
        print!("Enter your VRChat username: ");
        io::stdout().flush().map_err(|e| VRChatError(format!("Failed to flush stdout: {}", e)))?;
        let mut username = String::new();
        io::stdin().read_line(&mut username).map_err(|e| VRChatError(format!("Failed to read username: {}", e)))?;
        let username = username.trim();

        print!("Enter your VRChat password: ");
        io::stdout().flush().map_err(|e| VRChatError(format!("Failed to flush stdout: {}", e)))?;
        let password = read_password().map_err(|e| VRChatError(format!("Failed to read password: {}", e)))?;

        let resp = client.get("https://api.vrchat.cloud/api/1/auth/user")
            .basic_auth(username, Some(&password))
            .send()
            .await
            .map_err(|e| VRChatError(format!("Failed to send login request: {}", e)))?;

        println!("Login response status: {}", resp.status());

        if !resp.status().is_success() {
            return Err(VRChatError("Login failed".to_string()));
        }

        let auth_cookie = Self::extract_auth_cookie(resp.headers())
            .ok_or_else(|| VRChatError("No auth cookie found in login response".to_string()))?;

        // Check if 2FA is required
        let body: serde_json::Value = resp.json().await
            .map_err(|e| VRChatError(format!("Failed to parse response JSON: {}", e)))?;

        if let Some(two_factor_types) = body["requiresTwoFactorAuth"].as_array() {
            if two_factor_types.contains(&serde_json::json!("totp")) {
                println!("Two-factor authentication required.");
                return Self::handle_2fa_static(client, &auth_cookie).await;
            }
        }

        Ok(auth_cookie)
    }

    // The instance method for handling 2FA during get_current_user_id
    async fn handle_2fa(&self, auth_cookie: &str) -> Result<String, VRChatError> {
        Self::handle_2fa_static(&self.client, auth_cookie).await
    }


    async fn handle_2fa_static(client: &Client, auth_cookie: &str) -> Result<String, VRChatError> {
        print!("Enter your 2FA code: ");
        io::stdout().flush().map_err(|e| VRChatError(format!("Failed to flush stdout: {}", e)))?;
        let mut twofa_code = String::new();
        io::stdin().read_line(&mut twofa_code).map_err(|e| VRChatError(format!("Failed to read 2FA code: {}", e)))?;
        let twofa_code = twofa_code.trim();

        let twofa_resp = client.post("https://api.vrchat.cloud/api/1/auth/twofactorauth/totp/verify")
            .header(COOKIE, auth_cookie)
            .json(&serde_json::json!({
                "code": twofa_code
            }))
            .send()
            .await
            .map_err(|e| VRChatError(format!("Failed to send 2FA verification request: {}", e)))?;

        info!("2FA response status: {}", twofa_resp.status());

        if !twofa_resp.status().is_success() {
            let error_body = twofa_resp.text().await
                .map_err(|e| VRChatError(format!("Failed to read 2FA error response: {}", e)))?;
            return Err(VRChatError(format!("2FA verification failed: {}", error_body)));
        }

        // The original auth cookie is still valid after 2FA
        Ok(auth_cookie.to_string())
    }

    fn extract_auth_cookie(headers: &HeaderMap<HeaderValue>) -> Option<String> {
        headers.get_all("set-cookie")
            .iter()
            .find_map(|value| {
                let cookie_str = value.to_str().ok()?;
                if cookie_str.contains("auth=") {
                    Some(cookie_str.to_string())
                } else {
                    None
                }
            })
    }

    pub async fn get_current_user_id(&self) -> Result<String, VRChatError> {
        if let Some(id) = self.current_user_id.read().await.clone() {
            return Ok(id);
        }

        let mut attempts = 0;
        loop {
            let auth_cookie = self.auth_cookie.read().await.clone();
            let response = self.client.get("https://api.vrchat.cloud/api/1/auth/user")
                .header(COOKIE, &auth_cookie)
                .header(USER_AGENT, "kittynvrc/twitchbot")
                .send()
                .await
                .map_err(|e| VRChatError(format!("Failed to send request: {}", e)))?;

            info!("User info response status: {}", response.status());

            let body = response.text().await
                .map_err(|e| VRChatError(format!("Failed to get response body: {}", e)))?;

            let user_info: Value = serde_json::from_str(&body)
                .map_err(|e| VRChatError(format!("Failed to parse JSON: {}", e)))?;

            if let Some(two_factor_types) = user_info["requiresTwoFactorAuth"].as_array() {
                if two_factor_types.contains(&Value::String("totp".to_string())) {
                    println!("Two-factor authentication required.");
                    let new_auth_cookie = self.handle_2fa(&auth_cookie).await?;
                    *self.auth_cookie.write().await = new_auth_cookie.clone(); // Changed to write()
                    let mut config = self.config.write().await;
                    config.set_vrchat_auth_cookie(new_auth_cookie)?;
                    continue;
                }
            }

            if let Some(id) = user_info.get("id").and_then(|id| id.as_str()) {
                let id = id.to_string();
                *self.current_user_id.write().await = Some(id.clone());
                return Ok(id);
            } else if user_info.get("error").is_some() {
                warn!("Authentication failed. Attempting to log in again.");
                let new_auth_cookie = Self::login(&self.client).await?;
                *self.auth_cookie.write().await = new_auth_cookie.clone(); // Changed to write()
                let mut config = self.config.write().await;
                config.set_vrchat_auth_cookie(new_auth_cookie)?;
                attempts += 1;
                if attempts >= 3 {
                    return Err(VRChatError("Failed to authenticate after 3 attempts".to_string()));
                }
                continue;
            } else {
                return Err(VRChatError("Failed to get user ID from response".to_string()));
            }
        }
    }

    pub async fn get_auth_cookie(&self) -> String {
        self.auth_cookie.read().await.clone()
    }

    pub async fn disconnect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Implement VRChat disconnection logic here
        // For example:
        // self.websocket.close().await?;
        // self.clear_session_data()?;
        info!("VRChat client disconnected");
        Ok(())
    }
}

impl VRChatClient
{
    pub async fn update_current_world(&self, world: World) -> Result<(), VRChatError> {
        // Store the new world information
        *self.current_world.write().await = Some(world.clone());

        // Send the updated world information to the frontend via WebSocket
        let message = WebSocketMessage {
            module: "vrchat".to_string(),
            action: "world_update".to_string(),
            data: serde_json::to_value(world.clone()).map_err(|e| VRChatError(format!("Failed to serialize world data: {}", e)))?,
        };
        self.websocket_tx.send(message)
            .map_err(|e| VRChatError(format!("Failed to send world update via WebSocket: {}", e)))?;

        info!("Current world updated and sent to frontend: {:?}", world);
        Ok(())
    }

    pub async fn get_current_world(&self) -> Result<World, VRChatError> {
        self.current_world.read().await
            .clone()
            .ok_or_else(|| VRChatError("No current world data available".to_string()))
    }

    pub async fn get_friends(&self) -> Result<Vec<Friend>, VRChatError> {
        // Implement actual friend fetching logic here
        Err(VRChatError("Friend fetching not implemented yet".to_string()))
    }

    pub async fn join_world(&self, world_id: &str) -> Result<(), VRChatError> {
        info!("Attempting to join world: {}", world_id);
        // Implement actual world joining logic here
        Err(VRChatError("World joining not implemented yet".to_string()))
    }

    pub async fn get_status(&self) -> Result<VRChatStatus, VRChatError> {
        Ok(VRChatStatus {
            online: self.is_online().await,
            current_world: self.get_current_world().await.ok(),
            friend_count: self.get_friends().await.map(|f| f.len()).unwrap_or(0),
        })
    }
}