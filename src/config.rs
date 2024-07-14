use std::path::Path;
use std::fs;
use serde::{Deserialize, Serialize};
use config::{Config as ConfigCrate, File, FileFormat};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub twitch_username: Option<String>,
    pub twitch_token: Option<String>,
    pub twitch_channel: Option<String>,
    pub vrchat_auth_cookie: Option<String>,
    // Fields for future expansion
    pub twitch_client_id: Option<String>,
    pub twitch_client_secret: Option<String>,
    pub discord_token: Option<String>,
}

impl Config {
    const CONFIG_PATH: &'static str = "mewbot.conf";

    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if !Path::new(Self::CONFIG_PATH).exists() {
            Self::create_empty_config(Self::CONFIG_PATH)?;
        }

        let config = ConfigCrate::builder()
            .add_source(File::new(Self::CONFIG_PATH, FileFormat::Toml))
            .build()?;

        config.try_deserialize().map_err(Into::into)
    }

    fn create_empty_config(path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let empty_config = Config {
            twitch_username: None,
            twitch_token: None,
            twitch_channel: None,
            vrchat_auth_cookie: None,
            twitch_client_id: None,
            twitch_client_secret: None,
            discord_token: None,
        };

        let toml = toml::to_string(&empty_config)?;
        fs::write(path, toml)?;

        println!("Created empty config file at {}.", path);
        Ok(())
    }

    pub fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let toml = toml::to_string(self)?;
        fs::write(Self::CONFIG_PATH, toml)?;
        Ok(())
    }

    pub fn set_twitch_credentials(&mut self, username: String, token: String, channel: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.twitch_username = Some(username);
        self.twitch_token = Some(token);
        self.twitch_channel = Some(channel);
        self.update()
    }

    pub fn set_vrchat_auth_cookie(&mut self, auth_cookie: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.vrchat_auth_cookie = Some(auth_cookie);
        self.update()
    }
}