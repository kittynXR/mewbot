use std::sync::Arc;
use tokio::sync::RwLock;
use twitch_irc::message::PrivmsgMessage;
use twitch_irc::{SecureTCPTransport, TwitchIRCClient};
use twitch_irc::login::StaticLoginCredentials;
use crate::config::Config;
use crate::logging::Logger;
use crate::{log_info, log_debug, log_error, log_verbose};
use std::future::Future;
use std::pin::Pin;
use crate::LogLevel;

pub fn handle_debug<'a>(
    msg: &'a PrivmsgMessage,
    client: &'a Arc<TwitchIRCClient<SecureTCPTransport, StaticLoginCredentials>>,
    channel: &'a str,
    config: &'a Arc<RwLock<Config>>,
    logger: &'a Arc<Logger>,
    params: &'a [&'a str],
) -> Pin<Box<dyn Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
    Box::pin(async move {
        let mut config = config.write().await;
        let mut response = String::new();

        println!("Debug command received with params: {:?}", params);

        if params.is_empty() {
            // Toggle between INFO and DEBUG if no params
            let new_level = if config.log_level <= LogLevel::INFO { LogLevel::DEBUG } else { LogLevel::INFO };
            config.set_log_level(new_level)?;
            response = format!("Log level is now {:?}", new_level);
        } else {
            for &param in params {
                match param.to_lowercase().as_str() {
                    "verbose" => {
                        config.set_log_level(LogLevel::VERBOSE)?;
                        response += "Log level is now VERBOSE. ";
                    }
                    "debug" => {
                        config.set_log_level(LogLevel::DEBUG)?;
                        response += "Log level is now DEBUG. ";
                    }
                    "info" => {
                        config.set_log_level(LogLevel::INFO)?;
                        response += "Log level is now INFO. ";
                    }
                    "status" => {
                        response += &format!("Current log level: {:?}. ", config.log_level);
                    }
                    _ => {
                        response += &format!("Unknown option: {}. ", param);
                    }
                }
            }
        }

        if response.is_empty() {
            response = "No changes were made.".to_string();
        }

        println!("{}", response);

        drop(config); // Release the write lock before logging

        log_info!(logger, "{}", response);
        log_debug!(logger, "This is a debug message");
        log_verbose!(logger, "This is a verbose message");

        client.say(channel.to_string(), format!("@{}, {}", msg.sender.name, response.trim())).await?;

        Ok(())
    })
}