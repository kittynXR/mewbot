use std::net::SocketAddr;
use warp::Filter;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::config::Config;
use crate::storage::StorageClient;
use crate::logging::Logger;
use serde_json::json;
use crate::{log_error, log_info};
use crate::LogLevel;
use crate::web_ui::storage_ext::StorageClientExt;
use crate::web_ui::websocket::DashboardState;

pub fn api_routes(
    config: Arc<RwLock<Config>>,
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>,
    dashboard_state: Arc<RwLock<DashboardState>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("api").and(
        start_bot(config.clone(), logger.clone())
            .or(stop_bot(config.clone(), logger.clone()))
            .or(update_settings(config.clone(), logger.clone()))
            .or(get_bot_status(config.clone(), storage.clone(), logger.clone(), dashboard_state.clone()))
            .or(get_user_list(storage.clone(), logger.clone()))
            .or(get_recent_messages(storage.clone(), logger.clone()))
            .or(get_twitch_channel(config.clone(), logger.clone()))
            .or(get_twitch_parent(config.clone(), logger.clone()))
            .or(get_config(config.clone(), logger.clone()))
            .or(update_config(config.clone(), logger.clone()))
    )
}

fn with_dashboard_state(
    dashboard_state: Arc<RwLock<DashboardState>>,
) -> impl Filter<Extract = (Arc<RwLock<DashboardState>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || dashboard_state.clone())
}

pub fn with_config(
    config: Arc<RwLock<Config>>,
) -> impl Filter<Extract = (Arc<RwLock<Config>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || config.clone())
}

pub fn with_storage(
    storage: Arc<RwLock<StorageClient>>,
) -> impl Filter<Extract = (Arc<RwLock<StorageClient>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || storage.clone())
}

pub fn with_logger(
    logger: Arc<Logger>,
) -> impl Filter<Extract = (Arc<Logger>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || logger.clone())
}

fn start_bot(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("start-bot")
        .and(warp::post())
        .and(with_config(config))
        .and(with_logger(logger))
        .and_then(handle_start_bot)
}

fn stop_bot(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("stop-bot")
        .and(warp::post())
        .and(with_config(config))
        .and(with_logger(logger))
        .and_then(handle_stop_bot)
}

fn update_settings(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("update-settings")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_config(config))
        .and(with_logger(logger))
        .and_then(handle_update_settings)
}

fn get_bot_status(
    config: Arc<RwLock<Config>>,
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>,
    dashboard_state: Arc<RwLock<DashboardState>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("bot-status")
        .and(warp::get())
        .and(with_config(config))
        .and(with_storage(storage))
        .and(with_logger(logger))
        .and(with_dashboard_state(dashboard_state))
        .and_then(handle_get_bot_status)
}

fn get_user_list(
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("user-list")
        .and(warp::get())
        .and(with_storage(storage))
        .and(with_logger(logger))
        .and_then(handle_get_user_list)
}

fn get_recent_messages(
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("recent-messages")
        .and(warp::get())
        .and(with_storage(storage))
        .and(with_logger(logger))
        .and_then(handle_get_recent_messages)
}

fn get_twitch_channel(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("twitch-channel")
        .and(warp::get())
        .and(with_config(config))
        .and(with_logger(logger))
        .and_then(handle_get_twitch_channel)
}

fn get_twitch_parent(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("twitch-parent")
        .and(warp::get())
        .and(with_config(config))
        .and(with_logger(logger))
        .and_then(handle_get_twitch_parent)
}

fn get_config(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("config")
        .and(warp::get())
        .and(with_config(config))
        .and(with_logger(logger))
        .and_then(handle_get_config)
}

fn update_config(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path("config")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_config(config))
        .and(with_logger(logger))
        .and_then(handle_update_config)
}

async fn handle_get_config(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> Result<impl warp::Reply, warp::Rejection> {
    log_info!(logger, "Fetching configuration");
    let config = config.read().await;
    Ok(warp::reply::json(&*config))
}

async fn handle_update_config(
    new_config: Config,
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> Result<impl warp::Reply, warp::Rejection> {
    log_info!(logger, "Updating configuration");
    let mut config_write = config.write().await;
    *config_write = new_config;
    if let Err(e) = config_write.save() {
        log_error!(logger, "Failed to save configuration: {:?}", e);
        return Err(warp::reject::custom(ApiError::ConfigUpdateError));
    }
    Ok(warp::reply::json(&json!({"status": "Configuration updated successfully"})))
}

async fn handle_get_twitch_parent(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let config = config.read().await;
    let hostname = config.web_ui_host.clone().unwrap_or_else(|| "localhost".to_string());

    let allowed_parents = vec![hostname];

    log_info!(logger, "Fetching Twitch parents: {:?}", allowed_parents);
    Ok(warp::reply::json(&serde_json::json!({ "parents": allowed_parents })))
}

async fn handle_get_twitch_channel(
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> Result<impl warp::Reply, warp::Rejection> {
    let config = config.read().await;
    let channel = config.twitch_channel_to_join.clone().unwrap_or_default();
    log_info!(logger, "Fetching Twitch channel: {}", channel);
    Ok(warp::reply::json(&serde_json::json!({ "channel": channel })))
}

async fn handle_start_bot(config: Arc<RwLock<Config>>, logger: Arc<Logger>) -> Result<impl warp::Reply, warp::Rejection> {
    // Implement logic to start the bot
    log_info!(logger, "Starting bot...");
    // TODO: Implement actual bot start logic
    Ok(warp::reply::json(&json!({"status": "Bot started"})))
}

async fn handle_stop_bot(config: Arc<RwLock<Config>>, logger: Arc<Logger>) -> Result<impl warp::Reply, warp::Rejection> {
    // Implement logic to stop the bot
    log_info!(logger, "Stopping bot...");
    // TODO: Implement actual bot stop logic
    Ok(warp::reply::json(&json!({"status": "Bot stopped"})))
}

async fn handle_update_settings(
    new_settings: serde_json::Value,
    config: Arc<RwLock<Config>>,
    logger: Arc<Logger>,
) -> Result<impl warp::Reply, warp::Rejection> {
    // Implement logic to update settings
    log_info!(logger, "Updating settings: {:?}", new_settings);
    // TODO: Implement actual settings update logic
    Ok(warp::reply::json(&json!({"status": "Settings updated"})))
}

async fn handle_get_bot_status(
    config: Arc<RwLock<Config>>,
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>,
    dashboard_state: Arc<RwLock<DashboardState>>,
) -> Result<impl warp::Reply, warp::Rejection> {
    log_info!(logger, "Fetching bot status");
    let dashboard_state = dashboard_state.read().await;
    let bot_status = dashboard_state.bot_status.read().await;

    let status = json!({
        "status": if bot_status.is_online() { "online" } else { "offline" },
        "uptime": bot_status.uptime_string(),
        "active_modules": ["twitch", "discord", "vrchat"] // You may want to implement this properly
    });
    log_info!(logger, "Bot status: {:?}", status);
    Ok(warp::reply::json(&status))
}

async fn handle_get_user_list(
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>,
) -> Result<impl warp::Reply, warp::Rejection> {
    log_info!(logger, "Fetching user list");
    let storage = storage.read().await;
    match storage.get_user_list().await {
        Ok(users) => Ok(warp::reply::json(&users)),
        Err(e) => {
            log_error!(logger, "Error fetching user list: {:?}", e);
            Err(warp::reject::custom(ApiError::DatabaseError))
        }
    }
}

async fn handle_get_recent_messages(
    storage: Arc<RwLock<StorageClient>>,
    logger: Arc<Logger>,
) -> Result<impl warp::Reply, warp::Rejection> {
    log_info!(logger, "Fetching recent messages");
    let storage = storage.read().await;
    match storage.get_recent_messages(10).await {
        Ok(messages) => Ok(warp::reply::json(&messages)),
        Err(e) => {
            log_error!(logger, "Error fetching recent messages: {:?}", e);
            Err(warp::reject::custom(ApiError::DatabaseError))
        }
    }
}

#[derive(Debug)]
enum ApiError {
    DatabaseError,
    ConfigUpdateError,
}

impl warp::reject::Reject for ApiError {}