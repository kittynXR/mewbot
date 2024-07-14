use crate::vrchat::models::{VRChatError};
use crate::Config;
use reqwest::Client;
use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, COOKIE};
use serde_json::Value;
use std::io::{self, Write};
use rpassword::read_password;

pub struct VRChatClient {
    client: Client,
    auth_cookie: String,
    config: Config,
}

impl VRChatClient {
    pub async fn new(config: &mut Config) -> Result<Self, VRChatError> {
        let client = Client::builder()
            .user_agent("kittynvrc/twitchbot")
            .build()
            .map_err(|e| VRChatError(format!("Failed to build client: {}", e)))?;

        let auth_cookie = match &config.vrchat_auth_cookie {
            Some(cookie) => cookie.clone(),
            None => {
                let cookie = Self::login(&client).await?;
                config.set_vrchat_auth_cookie(cookie.clone())?;
                cookie
            }
        };

        Ok(VRChatClient {
            client,
            auth_cookie,
            config: config.clone(),
        })
    }

    pub async fn get_current_user_id(&mut self) -> Result<String, VRChatError> {
        loop {
            let response = self.client.get("https://api.vrchat.cloud/api/1/auth/user")
                .header(COOKIE, &self.auth_cookie)
                .header(USER_AGENT, "kittynvrc/twitchbot")
                .send()
                .await
                .map_err(|e| VRChatError(format!("Failed to send request: {}", e)))?;

            println!("User info response status: {}", response.status());

            let body = response.text().await
                .map_err(|e| VRChatError(format!("Failed to get response body: {}", e)))?;

            println!("Response body: {}", body); // Debug print

            let user_info: Value = serde_json::from_str(&body)
                .map_err(|e| VRChatError(format!("Failed to parse JSON: {}", e)))?;

            if let Some(two_factor_types) = user_info["requiresTwoFactorAuth"].as_array() {
                if two_factor_types.contains(&Value::String("totp".to_string())) {
                    println!("Two-factor authentication required.");
                    self.auth_cookie = self.handle_2fa(&self.auth_cookie).await?;
                    self.config.set_vrchat_auth_cookie(self.auth_cookie.clone())?;
                    continue;
                }
            }

            if let Some(id) = user_info.get("id").and_then(|id| id.as_str()) {
                return Ok(id.to_string());
            } else if user_info.get("error").is_some() {
                println!("Authentication failed. Attempting to log in again.");
                self.auth_cookie = Self::login(&self.client).await?;
                self.config.set_vrchat_auth_cookie(self.auth_cookie.clone())?;
                continue;
            } else {
                return Err(VRChatError("Failed to get user ID from response".to_string()));
            }
        }
    }

    async fn handle_2fa(&self, auth_cookie: &str) -> Result<String, VRChatError> {
        print!("Enter your 2FA code: ");
        io::stdout().flush().map_err(|e| VRChatError(format!("Failed to flush stdout: {}", e)))?;
        let mut twofa_code = String::new();
        io::stdin().read_line(&mut twofa_code).map_err(|e| VRChatError(format!("Failed to read 2FA code: {}", e)))?;
        let twofa_code = twofa_code.trim();

        let twofa_resp = self.client.post("https://api.vrchat.cloud/api/1/auth/twofactorauth/totp/verify")
            .header(COOKIE, auth_cookie)
            .json(&serde_json::json!({
                "code": twofa_code
            }))
            .send()
            .await
            .map_err(|e| VRChatError(format!("Failed to send 2FA verification request: {}", e)))?;

        println!("2FA response status: {}", twofa_resp.status());

        if !twofa_resp.status().is_success() {
            let error_body = twofa_resp.text().await
                .map_err(|e| VRChatError(format!("Failed to read 2FA error response: {}", e)))?;
            return Err(VRChatError(format!("2FA verification failed: {}", error_body)));
        }

        // If 2FA is successful, the original auth cookie is still valid
        Ok(auth_cookie.to_string())
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

        Self::extract_auth_cookie(resp.headers())
            .ok_or_else(|| VRChatError("No auth cookie found in login response".to_string()))
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

    pub fn get_auth_cookie(&self) -> String {
        self.auth_cookie.clone()
    }
}