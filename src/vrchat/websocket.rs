use crate::vrchat::models::{VRChatError, World};
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use chrono::{DateTime, Utc};
use tokio::sync::{Mutex, RwLock};
use tokio::time::sleep;
use tokio_tungstenite::{connect_async_tls_with_config, Connector};
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;
use tokio_tungstenite::tungstenite::http::{Request, Uri};
use tokio_tungstenite::tungstenite::http::header;
use tokio::sync::mpsc;
use crate::log_info;
use crate::vrchat::VRChatClient;
use crate::web_ui::websocket::{DashboardState, WebSocketMessage};

pub async fn handler(
    auth_cookie: String,
    world_info: Arc<Mutex<Option<World>>>,
    current_user_id: String,
    vrchat_client: Arc<VRChatClient>,
    dashboard_state: Arc<RwLock<DashboardState>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut delay = Duration::from_secs(1);
    let max_delay = Duration::from_secs(64);

    loop {
        match connect_to_websocket(&auth_cookie).await {
            Ok(mut ws_stream) => {
                println!("WebSocket connection established");
                dashboard_state.write().await.update_vrchat_status(true);

                while let Some(message) = ws_stream.next().await {
                    match message {
                        Ok(TungsteniteMessage::Text(msg)) => {
                            if let Ok(Some(world)) = extract_user_location_info(&msg, &current_user_id) {
                                let mut dashboard = dashboard_state.write().await;
                                dashboard.update_vrchat_world(Some(world.clone()));
                                println!("Current user changed world: {:?}", world);
                                println!("Current VRChat world state updated: {:?}", dashboard.vrchat_world);

                                // Update the separate world_info as well
                                let mut guard = world_info.lock().await;
                                *guard = Some(world);
                            }
                        }
                        Err(err) => {
                            println!("WebSocket error: {}", err);
                            break;
                        }
                        _ => {}
                    }
                }
                delay = Duration::from_secs(1);
            }
            Err(err) => {
                println!("Failed to connect: {}", err);
                dashboard_state.write().await.update_vrchat_status(false);
                delay = std::cmp::min(delay * 2, max_delay);
            }
        }

        println!("Attempting to reconnect after {:?}", delay);
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

fn extract_user_location_info(json_message: &str, current_user_id: &str) -> Result<Option<World>, VRChatError> {
    let message: serde_json::Value = serde_json::from_str(json_message)
        .map_err(|e| VRChatError(format!("Failed to parse JSON: {}", e)))?;

    if let Some(content) = message.get("content") {
        let content: serde_json::Value = serde_json::from_str(content.as_str().unwrap_or(""))
            .map_err(|e| VRChatError(format!("Failed to parse content JSON: {}", e)))?;

        if let Some(user_id) = content.get("userId") {
            let user_id_str = user_id.as_str().unwrap_or("");
            if user_id_str != current_user_id {
                return Ok(None);
            }
        } else {
            return Ok(None);
        }

        if let Some(location) = content.get("location") {
            if location.as_str() == Some("private") {
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
            println!("Current user changed world: {:?}", world);
            return Ok(Some(world));
        }
    }

    Ok(None)
}