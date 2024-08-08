use std::path::Path;
use std::fs;
use std::io::{self, Write};
use serde::{Deserialize, Serialize};
use serenity::all::standard::Reason::Log;
use crate::logging::LogLevel;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub twitch_bot_username: Option<String>,
    pub twitch_user_id: Option<String>,
    pub twitch_channel_to_join: Option<String>,
    pub twitch_client_id: Option<String>,
    pub twitch_client_secret: Option<String>,
    pub twitch_irc_oauth_token: Option<String>,
    pub twitch_access_token: Option<String>,
    pub twitch_refresh_token: Option<String>,
    pub vrchat_auth_cookie: Option<String>,
    pub discord_token: Option<String>,
    pub discord_client_id: Option<String>,
    pub discord_guild_id: Option<String>,
    pub openai_secret: Option<String>,
    pub anthropic_secret: Option<String>,
    #[serde(default)]
    pub log_level: LogLevel,
    pub web_ui_port: Option<u16>,
}

impl Config {
    const CONFIG_PATH: &'static str = "mewbot.conf";

    pub fn new() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if Path::new(Self::CONFIG_PATH).exists() {
            let mut config: Config = toml::from_str(&fs::read_to_string(Self::CONFIG_PATH)?)?;
            config.prompt_for_missing_fields()?;
            Ok(config)
        } else {
            Self::initial_setup()
        }
    }

    fn prompt_for_missing_fields(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Twitch IRC
        if self.twitch_bot_username.is_none() {
            self.twitch_bot_username = Some(Self::prompt_input("Enter your Twitch IRC username: ")?);
        }
        if self.twitch_irc_oauth_token.is_none() {
            self.twitch_irc_oauth_token = Some(Self::prompt_input("Enter your Twitch IRC OAuth Token: ")?);
        }
        if self.twitch_channel_to_join.is_none() {
            self.twitch_channel_to_join = Some(Self::prompt_input("Enter the Twitch channel to join: ")?);
        }

        // Twitch API
        if self.twitch_client_id.is_none() {
            self.twitch_client_id = Some(Self::prompt_input("Enter your Twitch API Client ID: ")?);
        }
        if self.twitch_client_secret.is_none() {
            self.twitch_client_secret = Some(Self::prompt_input("Enter your Twitch API Client Secret: ")?);
        }

        // Discord
        if self.discord_token.is_none() {
            self.discord_token = Some(Self::prompt_input("Enter your Discord Bot Token (leave empty if not using Discord): ")?);
        }

        // OpenAI
        if self.openai_secret.is_none() {
            self.openai_secret = Some(Self::prompt_input("Enter your OpenAI API secret key (leave empty if not using OpenAI): ")?);
        }

        // Anthropic
        if self.anthropic_secret.is_none() {
            self.anthropic_secret = Some(Self::prompt_input("Enter your Anthropic API secret key (leave empty if not using Anthropic): ")?);
        }

        if self.discord_token.is_none() {
            self.discord_token = Some(Self::prompt_input("Enter your Discord Bot Token: ")?);
        }
        if self.discord_client_id.is_none() {
            self.discord_client_id = Some(Self::prompt_input("Enter your Discord Application ID: ")?);
        }
        if self.discord_guild_id.is_none() {
            self.discord_guild_id = Some(Self::prompt_input("Enter the Discord Guild ID where the bot will operate: ")?);
        }

        self.save()?;
        Ok(())
    }

    fn initial_setup() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        println!("Welcome to MewBot! Let's set up your configuration.");

        // Twitch Developer Console instructions
        println!("First, you'll need to create a Twitch application to get your Client ID and Client Secret.");
        println!("Please follow these steps:");
        println!("1. Go to https://dev.twitch.tv/console");
        println!("2. Log in with your Twitch account");
        println!("3. Click on 'Register Your Application'");
        println!("4. Fill in the required fields:");
        println!("   - Name: Choose a name for your application");
        println!("   - OAuth Redirect URLs: http://localhost:3000");
        println!("   - Category: Chat Bot");
        println!("5. Click 'Create'");
        println!("6. On the next page, you'll see your Client ID and you can generate a Client Secret");
        println!("\nPress Enter when you're ready to continue...");
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer)?;

        let twitch_client_id = Self::prompt_input("Enter your Twitch Client ID: ")?;
        let twitch_client_secret = Self::prompt_input("Enter your Twitch Client Secret: ")?;

        // Twitch Chat OAuth Token instructions
        println!("\nNow, let's get your Twitch Chat OAuth Token.");
        println!("Please follow these steps:");
        println!("1. Go to https://twitchapps.com/tmi/");
        println!("2. Click on 'Connect with Twitch'");
        println!("3. Authorize the application");
        println!("4. Copy the OAuth token (including the 'oauth:' prefix)");
        println!("\nPress Enter when you're ready to continue...");
        io::stdin().read_line(&mut buffer)?;

        let twitch_irc_oauth_token = Self::prompt_input("Enter your Twitch Chat OAuth Token: ")?;
        let twitch_bot_username = Self::prompt_input("Enter the username of your Twitch bot: ")?;
        let twitch_channel_to_join = Self::prompt_input("Enter the Twitch channel you want the bot to join: ")?;

        let twitch_irc_oauth_token = Self::prompt_input("Enter your Twitch Chat OAuth Token: ")?;
        let twitch_bot_username = Self::prompt_input("Enter the username of your Twitch bot: ")?;
        let twitch_channel_to_join = Self::prompt_input("Enter the Twitch channel you want the bot to join: ")?;

        println!("\nNow, let's set up your Discord bot.");
        println!("Please follow these steps:");
        println!("1. Go to https://discord.com/developers/applications");
        println!("2. Click on 'New Application' and give it a name");
        println!("3. After creating, go to the 'Bot' tab and click 'Add Bot'");
        println!("4. Under the bot's username, you'll see the Application ID - copy this");
        println!("5. Click on 'Reset Token' to generate a new token, then copy it");
        println!("6. Go to the 'OAuth2' tab, then 'URL Generator'");
        println!("7. Select 'bot' and 'applications.commands' scopes");
        println!("8. Select the necessary bot permissions (e.g., Send Messages, Manage Roles, etc.)");
        println!("9. Copy the generated URL and use it to invite the bot to your server");
        println!("10. In Discord, enable Developer Mode (User Settings > Advanced)");
        println!("11. Right-click on your server and select 'Copy ID' to get the Guild ID");
        println!("\nPress Enter when you're ready to continue...");
        let mut buffer = String::new();
        io::stdin().read_line(&mut buffer)?;

        let discord_token = Self::prompt_input("Enter your Discord Bot Token: ")?;
        let discord_client_id = Self::prompt_input("Enter your Discord Application ID: ")?;
        let discord_guild_id = Self::prompt_input("Enter the Discord Guild ID where the bot will operate: ")?;

        // Add prompts for OpenAI and Anthropic keys
        let openai_secret = Self::prompt_input("Enter your OpenAI API secret key (leave empty if not using OpenAI): ")?;
        let anthropic_secret = Self::prompt_input("Enter your Anthropic API secret key (leave empty if not using Anthropic): ")?;

        let web_ui_port = Self::prompt_input("Enter the port for the Web UI (default is 3000): ")?;
        let web_ui_port = if web_ui_port.is_empty() {
            Some(3000)
        } else {
            Some(web_ui_port.parse().unwrap_or(3000))
        };

        let config = Config {
            twitch_bot_username: Some(twitch_bot_username),
            twitch_channel_to_join: Some(twitch_channel_to_join),
            twitch_client_id: Some(twitch_client_id),
            twitch_client_secret: Some(twitch_client_secret),
            twitch_irc_oauth_token: Some(twitch_irc_oauth_token),
            twitch_access_token: None,
            twitch_refresh_token: None,
            twitch_user_id: None,
            vrchat_auth_cookie: None,
            discord_token: Some(discord_token),
            discord_client_id: Some(discord_client_id),
            discord_guild_id: Some(discord_guild_id),
            openai_secret: if openai_secret.is_empty() { None } else { Some(openai_secret) },
            anthropic_secret: if anthropic_secret.is_empty() { None } else { Some(anthropic_secret) },
            log_level: LogLevel::INFO,
            web_ui_port: web_ui_port,
        };

        config.save()?;
        println!("Configuration saved successfully!");

        Ok(config)
    }

    fn prompt_input(prompt: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        print!("{}", prompt);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_string())
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let toml = toml::to_string(self)?;
        let path = std::path::Path::new("mewbot.conf");
        std::fs::write(path, toml)?;
        println!("Config saved to: {:?}", path.canonicalize()?);
        Ok(())
    }

    pub fn set_twitch_tokens(&mut self, access_token: String, refresh_token: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.twitch_access_token = Some(access_token);
        self.twitch_refresh_token = Some(refresh_token);
        self.save()
    }

    pub fn set_vrchat_auth_cookie(&mut self, auth_cookie: String) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.vrchat_auth_cookie = Some(auth_cookie);
        self.save()
    }

    pub fn is_twitch_irc_configured(&self) -> bool {
        self.twitch_bot_username.is_some() &&
            self.twitch_irc_oauth_token.is_some() &&
            self.twitch_channel_to_join.is_some()
    }

    pub fn is_twitch_api_configured(&self) -> bool {
        self.twitch_client_id.is_some() &&
            self.twitch_client_secret.is_some()
    }

    pub fn is_vrchat_configured(&self) -> bool {
        self.vrchat_auth_cookie.is_some()
    }

    pub fn is_discord_configured(&self) -> bool {
        self.discord_token.is_some() &&
            self.discord_client_id.is_some() &&
            self.discord_guild_id.is_some()
    }

    pub fn set_log_level(&mut self, level: LogLevel) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.log_level = level;
        println!("Log level set to {:?}", self.log_level);
        self.save()?;
        Ok(())
    }
}