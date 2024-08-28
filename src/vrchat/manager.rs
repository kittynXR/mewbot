use std::sync::Arc;
use log::info;
use tokio::sync::RwLock;
use crate::vrchat::{VRChatClient, VRChatError, World};
use crate::web_ui::websocket::{DashboardState, WebSocketMessage};

pub struct VRChatManager {
    vrchat_client: Arc<VRChatClient>,
    dashboard_state: Arc<RwLock<DashboardState>>,
}

impl VRChatManager {
    pub fn new(vrchat_client: Arc<VRChatClient>, dashboard_state: Arc<RwLock<DashboardState>>) -> Self {
        Self {
            vrchat_client,
            dashboard_state,
        }
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Shutting down VRChatManager...");
        // Disconnect from VRChat
        self.vrchat_client.disconnect().await?;
        // Update dashboard state
        self.dashboard_state.write().await.update_vrchat_status(false);
        info!("VRChatManager shutdown complete.");
        Ok(())
    }

    pub async fn handle_message(&self, message: WebSocketMessage) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match message.action.as_str() {
            "get_current_world" => {
                let world = self.vrchat_client.get_current_world().await?;
                let response = WebSocketMessage {
                    module: "vrchat".to_string(),
                    action: "current_world".to_string(),
                    data: serde_json::to_value(world)?,
                };
                self.dashboard_state.write().await.broadcast_message(response).await?;
                Ok(())
            },
            "get_friends" => {
                let friends = self.vrchat_client.get_friends().await?;
                let response = WebSocketMessage {
                    module: "vrchat".to_string(),
                    action: "friends_list".to_string(),
                    data: serde_json::to_value(friends)?,
                };
                self.dashboard_state.write().await.broadcast_message(response).await?;
                Ok(())
            },
            "get_vrchat_status" => {
                let status = self.vrchat_client.get_status().await?;
                let response = WebSocketMessage {
                    module: "vrchat".to_string(),
                    action: "vrchat_status".to_string(),
                    data: serde_json::to_value(status)?,
                };
                self.dashboard_state.write().await.broadcast_message(response).await?;
                Ok(())
            },
            _ => Err(format!("Unknown VRChat action: {}", message.action).into()),
        }
    }

    pub async fn update_current_world(&self, world: World) -> Result<(), VRChatError> {
        self.vrchat_client.update_current_world(world).await;
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.vrchat_client.disconnect().await
    }

    pub async fn get_current_user_id(&self) -> Result<String, VRChatError> {
        self.vrchat_client.get_current_user_id().await
    }

    pub async fn get_auth_cookie(&self) -> String {
        self.vrchat_client.get_auth_cookie().await
    }

    pub async fn is_online(&self) -> bool {
        self.vrchat_client.is_online().await
    }

    pub async fn get_current_world(&self) -> Result<World, VRChatError> {
        self.vrchat_client.get_current_world().await
    }
}