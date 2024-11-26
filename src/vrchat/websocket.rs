use crate::vrchat::models::{VRChatError, World};
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use log::{debug, error, info, warn};
use tokio::sync::{RwLock};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async_tls_with_config, Connector};
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;
use tokio_tungstenite::tungstenite::http::{Request, Uri};
use tokio_tungstenite::tungstenite::http::header;
use crate::vrchat::{VRChatManager, VRChatMessage};
use crate::web_ui::websocket::{DashboardState};

pub async fn handler(
    auth_cookie: String,
    current_user_id: String,
    vrchat_manager: Arc<VRChatManager>,
    dashboard_state: Arc<RwLock<DashboardState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut delay = Duration::from_secs(1);
    let max_delay = Duration::from_secs(64);

    loop {
        info!("Attempting to connect to VRChat WebSocket");
        match connect_to_websocket(&auth_cookie).await {
            Ok(mut ws_stream) => {
                info!("WebSocket connection established");
                dashboard_state.write().await.update_vrchat_status(true).await;

                while let Some(message) = ws_stream.next().await {
                    match message {
                        Ok(TungsteniteMessage::Text(msg)) => {
                            match parse_vrchat_message(&msg, &current_user_id) {
                                Ok(Some(message_type)) => {
                                    match message_type {
                                        VRChatMessage::UserLocation(content) => {
                                            info!("Current user location update: {:?}", content);
                                            // Use await here
                                            match extract_user_location_info(&msg, &current_user_id).await {
                                                Ok(Some(new_world)) => {
                                                    info!("Current user entered new world: {:?}", new_world);

                                                    // Force invalidate cache first
                                                    if let Err(e) = vrchat_manager.force_invalidate_world_cache().await {
                                                        error!("Failed to invalidate world cache: {}", e);
                                                    }

                                                    // Then update the world information
                                                    if let Err(e) = vrchat_manager.update_current_world(new_world.clone()).await {
                                                        error!("Failed to update VRChatManager with new world: {}", e);
                                                    }

                                                    let mut dashboard = dashboard_state.write().await;
                                                    dashboard.update_vrchat_world(Some(new_world.clone())).await;
                                                    info!("Current VRChat world state updated: {:?}", dashboard.vrchat_world);
                                                }
                                                Ok(None) => {
                                                    debug!("No world update in location message");
                                                }
                                                Err(e) => {
                                                    error!("Failed to extract world info: {}", e);
                                                }
                                            }
                                        },
                                        VRChatMessage::UserOnline(_) => {
                                            info!("Current user is now online");
                                            dashboard_state.write().await.update_vrchat_status(true).await;
                                            if let Err(e) = vrchat_manager.connect_osc().await {
                                                error!("Failed to reestablish OSC connection: {}", e);
                                            } else {
                                                info!("OSC connection reestablished");
                                            }
                                        },
                                        VRChatMessage::UserOffline(_) => {
                                            info!("Current user is now offline");
                                            dashboard_state.write().await.update_vrchat_status(false).await;
                                            if let Err(e) = vrchat_manager.disconnect_osc().await {
                                                error!("Failed to close OSC connection: {}", e);
                                            } else {
                                                info!("OSC connection closed");
                                            }
                                        },
                                        VRChatMessage::Error(err) => {
                                            error!("Received error message: {:?}", err);
                                        },
                                        VRChatMessage::Unknown(_) => {
                                            debug!("Received unknown message type for current user");
                                        },
                                    }
                                },
                                Ok(None) => {
                                    // Message not related to current user, ignore
                                },
                                Err(e) => {
                                    error!("Failed to parse VRChat message: {}", e);
                                }
                            }
                        },
                        Ok(TungsteniteMessage::Close(frame)) => {
                            info!("WebSocket connection closed: {:?}", frame);
                            break;
                        },
                        Ok(_) => {
                            debug!("Received non-text WebSocket message");
                        },
                        Err(err) => {
                            error!("WebSocket error: {}", err);
                            break;
                        }
                    }
                }
                delay = Duration::from_secs(1);
            }
            Err(err) => {
                error!("Failed to connect to WebSocket: {}", err);
                dashboard_state.write().await.update_vrchat_status(false).await;
                delay = std::cmp::min(delay * 2, max_delay);
            }
        }

        warn!("WebSocket disconnected. Attempting to reconnect after {:?}", delay);
        sleep(delay).await;
    }
}

async fn connect_to_websocket(auth_cookie: &str) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, VRChatError> {
    let auth_token = extract_auth_token(auth_cookie)?;
    let request = create_request(&auth_token)?;
    let (ws_stream, _) = connect_async_tls_with_config(request, None, false, Some(Connector::NativeTls(native_tls::TlsConnector::new().unwrap())))
        .await
        .map_err(|e| VRChatError(format!("WebSocket connection failed: {}", e)))?;

    Ok(ws_stream)
}

fn extract_auth_token(auth_cookie: &str) -> Result<String, VRChatError> {
    auth_cookie
        .split(';')
        .find(|s| s.trim().starts_with("auth="))
        .and_then(|s| s.trim().strip_prefix("auth="))
        .ok_or_else(|| VRChatError("Failed to extract auth token from cookie".to_string()))
        .map(String::from)
}

fn create_request(auth_token: &str) -> Result<Request<()>, VRChatError> {
    let url: Uri = format!("wss://pipeline.vrchat.cloud/?authToken={}", auth_token)
        .parse()
        .map_err(|e| VRChatError(format!("Failed to parse WebSocket URL: {}", e)))?;

    let key = tokio_tungstenite::tungstenite::handshake::client::generate_key();

    Request::builder()
        .method("GET")
        .uri(url)
        .header(header::HOST, "pipeline.vrchat.cloud")
        .header(header::ORIGIN, "https://vrchat.com")
        .header(header::USER_AGENT, "kittynvrc/twitchbot")
        .header(header::CONNECTION, "Upgrade")
        .header(header::UPGRADE, "websocket")
        .header(header::SEC_WEBSOCKET_VERSION, "13")
        .header(header::SEC_WEBSOCKET_KEY, key)
        .body(())
        .map_err(|e| VRChatError(format!("Failed to build WebSocket request: {}", e)))
}

async fn extract_user_location_info(json_message: &str, current_user_id: &str) -> Result<Option<World>, VRChatError> {
    let message: serde_json::Value = serde_json::from_str(json_message)
        .map_err(|e| VRChatError(format!("Failed to parse JSON: {}", e)))?;

    if let Some(content) = message.get("content") {
        let content: serde_json::Value = serde_json::from_str(content.as_str().unwrap_or(""))
            .map_err(|e| VRChatError(format!("Failed to parse content JSON: {}", e)))?;

        // First check if this message is for the current user
        if let Some(user_id) = content.get("userId") {
            let user_id_str = user_id.as_str().unwrap_or("");
            if user_id_str != current_user_id {
                debug!("Location update for different user: {}", user_id_str);
                return Ok(None);
            }
        } else {
            debug!("No user ID in message");
            return Ok(None);
        }

        // Check if user is in private
        if let Some(location) = content.get("location") {
            if location.as_str() == Some("private") {
                info!("User entered private instance");
                return Ok(None);
            }
        }

        if let Some(world) = content.get("world") {
            let created_at = world.get("created_at")
                .and_then(|d| d.as_str())
                .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|| Utc::now());

            let updated_at = world.get("updated_at")
                .and_then(|d| d.as_str())
                .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|| Utc::now());

            let world = World {
                id: world.get("id").and_then(|id| id.as_str()).unwrap_or("").to_string(),
                name: world.get("name").and_then(|name| name.as_str()).unwrap_or("Unknown").to_string(),
                author_name: world.get("authorName").and_then(|name| name.as_str()).unwrap_or("Unknown").to_string(),
                capacity: world.get("capacity").and_then(|c| c.as_i64()).unwrap_or(0) as i32,
                description: world.get("description").and_then(|d| d.as_str()).unwrap_or("No description").to_string(),
                release_status: world.get("releaseStatus").and_then(|r| r.as_str()).unwrap_or("Unknown").to_string(),
                created_at,
                updated_at,
            };
            info!("Extracted world update: {:?}", world);
            return Ok(Some(world));
        }
    }

    Ok(None)
}

fn parse_vrchat_message(json_message: &str, current_user_id: &str) -> Result<Option<VRChatMessage>, VRChatError> {
    let message: serde_json::Value = serde_json::from_str(json_message)
        .map_err(|e| VRChatError(format!("Failed to parse JSON: {}", e)))?;

    if let Some(content) = message.get("content") {
        let content: serde_json::Value = serde_json::from_str(content.as_str().unwrap_or(""))
            .map_err(|e| VRChatError(format!("Failed to parse content JSON: {}", e)))?;

        if let Some(user_id) = content.get("userId").and_then(|id| id.as_str()) {
            if user_id == current_user_id {
                match message.get("type").and_then(|t| t.as_str()) {
                    Some("friend-online") | Some("friend-active") => {
                        return Ok(Some(VRChatMessage::UserOnline(content)));
                    },
                    Some("friend-offline") => {
                        return Ok(Some(VRChatMessage::UserOffline(content)));
                    },
                    Some("friend-location") => {
                        return Ok(Some(VRChatMessage::UserLocation(content)));
                    },
                    _ => {}
                }
            }
        }
    }

    Ok(None) // Return None for messages not related to the current user
}