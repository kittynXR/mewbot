use std::path::Path;
use std::fs;
use std::io::{self, Write};
use serde::{Deserialize, Serialize};
use log::LevelFilter;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SocialLinks {
    pub discord: Option<String>,
    pub xdotcom: Option<String>,
    pub vrchat_group: Option<String>,
    pub business_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OBSInstanceConfig {
    pub ip: String,
    pub port: u16,
    pub auth_required: bool,
    pub password: Option<String>,
    pub use_ssl: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OBSManagerConfig {
    pub is_dual_pc_setup: bool,
    pub instance1: OBSInstanceConfig,
    pub instance2: Option<OBSInstanceConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub twitch_bot_username: Option<String>,
    pub twitch_user_id: Option<String>,
    pub twitch_channel_to_join: Option<String>,
    pub twitch_client_id: Option<String>,
    pub twitch_client_secret: Option<String>,
    pub twitch_bot_oauth_token: Option<String>,
    pub twitch_broadcaster_oauth_token: Option<String>,
    pub twitch_access_token: Option<String>,
    pub twitch_refresh_token: Option<String>,
    pub vrchat_auth_cookie: Option<String>,
    pub discord_token: Option<String>,
    pub discord_client_id: Option<String>,
    pub discord_guild_id: Option<String>,
    pub openai_secret: Option<String>,
    pub anthropic_secret: Option<String>,
    #[serde(with = "log_level_serde")]
    pub log_level: LevelFilter,
    pub web_ui_host: Option<String>,
    pub web_ui_port: Option<u16>,
    #[serde(default = "default_additional_streams")]
    pub additional_streams: Vec<String>,
    #[serde(default)]
    pub social_links: SocialLinks,
    #[serde(default)]
    pub obs_manager: OBSManagerConfig,
}

fn default_additional_streams() -> Vec<String> {
    vec!["".to_string(); 4]
}

// Custom serialization for log::LevelFilter
mod log_level_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use log::LevelFilter;

    pub fn serialize<S>(level: &LevelFilter, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:?}", level))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<LevelFilter, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_uppercase().as_str() {
            "ERROR" => Ok(LevelFilter::Error),
            "WARN" => Ok(LevelFilter::Warn),
            "INFO" => Ok(LevelFilter::Info),
            "DEBUG" => Ok(LevelFilter::Debug),
            "TRACE" => Ok(LevelFilter::Trace),
            _ => Ok(LevelFilter::Info), // Default to Info if unrecognized
        }
    }
}

impl Default for SocialLinks {
    fn default() -> Self {
        SocialLinks {
            discord: None,
            xdotcom: None,
            vrchat_group: None,
            business_url: None,
        }
    }
}
impl Default for OBSInstanceConfig {
    fn default() -> Self {
        Self {
            ip: "127.0.0.1".to_string(),
            port: 4455,
            auth_required: false,
            password: None,
            use_ssl: false,
        }
    }
}
impl Default for OBSManagerConfig {
    fn default() -> Self {
        Self {
            is_dual_pc_setup: false,
            instance1: OBSInstanceConfig::default(),
            instance2: None,
        }
    }
}

impl Config {
    const CONFIG_PATH: &'static str = "C:\\Users\\kittyn\\RustroverProjects\\mewbot\\target\\debug\\mewbot.conf";

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
        if self.twitch_bot_oauth_token.is_none() {
            self.twitch_bot_oauth_token = Some(Self::prompt_input("Enter your Twitch Bot OAuth Token: ")?);
        }

        if self.twitch_broadcaster_oauth_token.is_none() {
            self.twitch_broadcaster_oauth_token = Some(Self::prompt_input("Enter your Twitch Bot OAuth Token: ")?);
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

        // OBS Manager Configuration
        if self.obs_manager.instance1.ip == "127.0.0.1" && self.obs_manager.instance1.port == 4455 && !self.obs_manager.is_dual_pc_setup {
            println!("\nLet's configure your OBS setup.");
            let setup_type = Self::prompt_input("Are you using a 1 PC or 2 PC streaming setup? (1/2): ")?;
            self.obs_manager.is_dual_pc_setup = setup_type == "2";

            // Configure Instance 1
            println!("\nConfiguring OBS Instance 1:");
            self.obs_manager.instance1.ip = Self::prompt_input_with_default("Enter IP address for OBS Instance 1", &self.obs_manager.instance1.ip)?;
            self.obs_manager.instance1.port = Self::prompt_input_with_default("Enter port for OBS Instance 1", &self.obs_manager.instance1.port.to_string())?.parse()?;

            // Configure Instance 2 if it's a dual PC setup
            if self.obs_manager.is_dual_pc_setup {
                println!("\nConfiguring OBS Instance 2:");
                let instance2 = self.obs_manager.instance2.get_or_insert(OBSInstanceConfig::default());
                instance2.ip = Self::prompt_input_with_default("Enter IP address for OBS Instance 2", &instance2.ip)?;
                instance2.port = Self::prompt_input_with_default("Enter port for OBS Instance 2", &instance2.port.to_string())?.parse()?;
            }
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

        if self.web_ui_host.is_none() {
            self.web_ui_host = Some(Self::prompt_input("Enter the host for the Web UI (default is localhost): ")?);
            if self.web_ui_host.as_ref().unwrap().is_empty() {
                self.web_ui_host = Some("localhost".to_string());
            }
        }

        if self.web_ui_port.is_none() {
            let port_input = Self::prompt_input("Enter the port for the Web UI (default is 3333): ")?;
            self.web_ui_port = Some(if port_input.is_empty() {
                3333
            } else {
                port_input.parse().unwrap_or(3333)
            });
        }

        if self.additional_streams.is_empty() {
            println!("Enter up to 4 additional Twitch streams to embed (leave empty to skip):");
            for i in 0..4 {
                let stream = Self::prompt_input(&format!("Additional stream {}: ", i + 1))?;
                if !stream.is_empty() {
                    self.additional_streams.push(stream);
                } else {
                    break;
                }
            }
        }

        // Prompt for missing social links
        self.social_links.discord = Self::prompt_social_link("Discord server invite link", self.social_links.discord.clone())?;
        self.social_links.xdotcom = Self::prompt_social_link("X (formerly Twitter) profile link", self.social_links.xdotcom.clone())?;
        self.social_links.vrchat_group = Self::prompt_social_link("VRChat group link", self.social_links.vrchat_group.clone())?;
        self.social_links.business_url = Self::prompt_social_link("Business website URL", self.social_links.business_url.clone())?;

        self.save()?;
        Ok(())
    }

    fn prompt_social_link(prompt: &str, current_value: Option<String>) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        if current_value.is_none() {
            let input = Self::prompt_input(&format!("Enter your {} (press Enter to skip): ", prompt))?;
            if !input.is_empty() {
                Ok(Some(input))
            } else {
                Ok(None)
            }
        } else {
            Ok(current_value)
        }
    }

    fn prompt_optional_input(prompt: &str) -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
        let input = Self::prompt_input(prompt)?;
        if input.is_empty() {
            Ok(None)
        } else {
            Ok(Some(input))
        }
    }

    fn prompt_input_with_default(prompt: &str, default: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        print!("{} (default: {}): ", prompt, default);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.is_empty() {
            Ok(default.to_string())
        } else {
            Ok(input.to_string())
        }
    }

    fn initial_setup() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        println!("Welcome to MewBot! Let's set up your initial configuration.");

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

        let twitch_bot_username = Self::prompt_input("Enter the username of your Twitch bot: ")?;
        let twitch_bot_oauth_token = Self::prompt_input("Enter your Twitch Chat OAuth Token: ")?;

        let twitch_channel_to_join = Self::prompt_input("Enter the Twitch channel you want the bot to join: ")?;
        let twitch_broadcaster_oauth_token = Self::prompt_input("Enter your broadcaster Twitch Chat OAuth Token: ")?;


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

        // OBS Manager Configuration
        println!("\nNow, let's configure your OBS setup.");
        let setup_type = Self::prompt_input("Are you using a 1 PC or 2 PC streaming setup? (1/2): ")?;
        let is_dual_pc_setup = setup_type == "2";

        // Configure Instance 1
        println!("\nConfiguring OBS Instance 1:");
        let instance1 = OBSInstanceConfig {
            ip: Self::prompt_input_with_default("Enter IP address for OBS Instance 1", "127.0.0.1")?,
            port: Self::prompt_input_with_default("Enter port for OBS Instance 1", "4455")?.parse()?,
            auth_required: false,
            password: None,
            use_ssl: false,
        };

        // Configure Instance 2 if it's a dual PC setup
        let instance2 = if is_dual_pc_setup {
            println!("\nConfiguring OBS Instance 2:");
            Some(OBSInstanceConfig {
                ip: Self::prompt_input_with_default("Enter IP address for OBS Instance 2", "10.0.0.1")?,
                port: Self::prompt_input_with_default("Enter port for OBS Instance 2", "4455")?.parse()?,
                auth_required: false,
                password: None,
                use_ssl: false,
            })
        } else {
            None
        };

        let obs_manager = OBSManagerConfig {
            is_dual_pc_setup,
            instance1,
            instance2,
        };

        let discord_token = Self::prompt_input("Enter your Discord Bot Token: ")?;
        let discord_client_id = Self::prompt_input("Enter your Discord Application ID: ")?;
        let discord_guild_id = Self::prompt_input("Enter the Discord Guild ID where the bot will operate: ")?;

        // Add prompts for OpenAI and Anthropic keys
        let openai_secret = Self::prompt_input("Enter your OpenAI API secret key (leave empty if not using OpenAI): ")?;
        let anthropic_secret = Self::prompt_input("Enter your Anthropic API secret key (leave empty if not using Anthropic): ")?;

        let web_ui_host = Self::prompt_input("Enter the host for the Web UI (default is localhost): ")?;
        let web_ui_host = if web_ui_host.is_empty() {
            Some("localhost".to_string())
        } else {
            Some(web_ui_host)
        };

        let web_ui_port = Self::prompt_input("Enter the port for the Web UI (default is 3333): ")?;
        let web_ui_port = if web_ui_port.is_empty() {
            Some(3333)
        } else {
            Some(web_ui_port.parse().unwrap_or(3333))
        };

        let mut additional_streams = vec!["".to_string(); 4];

        println!("\nNow, let's set up your social links. You can skip any by pressing Enter.");
        let social_links = SocialLinks {
            discord: Self::prompt_optional_input("Enter your Discord server invite link: ")?,
            xdotcom: Self::prompt_optional_input("Enter your X (formerly Twitter) profile link: ")?,
            vrchat_group: Self::prompt_optional_input("Enter your VRChat group link: ")?,
            business_url: Self::prompt_optional_input("Enter your business website URL: ")?,
        };

        println!("Enter up to 4 additional Twitch streams to embed (leave empty to skip):");

        for i in 0..4 {
            let stream = Self::prompt_input(&format!("Additional stream {}: ", i + 1))?;
            if !stream.is_empty() {
                additional_streams[i] = stream;
            } else {
                break;
            }
        }

        let config = Config {
            twitch_bot_username: Some(twitch_bot_username),
            twitch_channel_to_join: Some(twitch_channel_to_join),
            twitch_client_id: Some(twitch_client_id),
            twitch_client_secret: Some(twitch_client_secret),
            twitch_bot_oauth_token: Some(twitch_bot_oauth_token),
            twitch_broadcaster_oauth_token: Some(twitch_broadcaster_oauth_token),
            twitch_access_token: None,
            twitch_refresh_token: None,
            twitch_user_id: None,
            vrchat_auth_cookie: None,
            discord_token: Some(discord_token),
            discord_client_id: Some(discord_client_id),
            discord_guild_id: Some(discord_guild_id),
            openai_secret: if openai_secret.is_empty() { None } else { Some(openai_secret) },
            anthropic_secret: if anthropic_secret.is_empty() { None } else { Some(anthropic_secret) },
            log_level: LevelFilter::Trace,
            web_ui_host,
            web_ui_port,
            additional_streams,
            social_links,
            obs_manager,
        };

        config.save()?;
        println!("Configuration saved successfully!");
        println!("Bot token from config: {}...", &config.twitch_bot_oauth_token.as_ref().unwrap_or(&String::new())[..14]);
        println!("Broadcaster token from config: {}...", &config.twitch_broadcaster_oauth_token.as_ref().unwrap_or(&String::new())[..14]);
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
            self.twitch_bot_oauth_token.is_some() &&
            self.twitch_channel_to_join.is_some() &&
            self.twitch_broadcaster_oauth_token.is_some()
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

    pub fn set_log_level(&mut self, level: LevelFilter) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.log_level = level;
        println!("Log level set to {:?}", self.log_level);
        self.save()?;
        Ok(())
    }
}