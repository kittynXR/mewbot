use std::sync::Arc;
use log::info;
use tokio::sync::RwLock;
use crate::osc::OSCManager;
use crate::vrchat::{VRChatClient, VRChatError, World};
use crate::web_ui::websocket::{DashboardState, WebSocketMessage};

pub struct VRChatManager {
    vrchat_client: Arc<VRChatClient>,
    dashboard_state: Arc<RwLock<DashboardState>>,
    osc_manager: Option<Arc<OSCManager>>,
}

impl VRChatManager {
    pub fn new(
        vrchat_client: Arc<VRChatClient>,
        dashboard_state: Arc<RwLock<DashboardState>>,
        osc_manager: Option<Arc<OSCManager>>,
    ) -> Self {
        Self {
            vrchat_client,
            dashboard_state,
            osc_manager,
        }
    }

    pub async fn shutdown(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Shutting down VRChatManager...");
        // Disconnect from VRChat
        self.vrchat_client.disconnect().await?;
        // Update dashboard state
        self.dashboard_state.write().await.update_vrchat_status(false).await;
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
        let _ = self.vrchat_client.update_current_world(world).await;
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

    pub async fn force_invalidate_world_cache(&self) -> Result<(), VRChatError> {
        // VRChatClient is already an Arc, no need to check Some
        let vrchat_client = &self.vrchat_client;
        vrchat_client.force_invalidate_cache().await?;
        info!("Forcefully invalidated world cache");
        Ok(())
    }

    pub async fn get_current_world(&self) -> Result<World, VRChatError> {
        let vrchat_client = &self.vrchat_client;

        let should_fetch = vrchat_client.should_refresh_cache().await;

        if should_fetch {
            if let Ok(world) = vrchat_client.fetch_current_world_api().await {
                if let Some(world) = world {
                    self.update_current_world(world.clone()).await?;
                    return Ok(world);
                }
            }
        }

        // If we have cached data, return it
        vrchat_client.get_cached_world().await
            .ok_or_else(|| VRChatError("No world data available".to_string()))
    }

    pub async fn connect_osc(&self) -> Result<(), VRChatError> {
        match &self.osc_manager {
            Some(osc_manager) => {
                osc_manager.connect().await.map_err(|e| VRChatError(format!("Failed to connect OSC: {}", e)))
            },
            None => Err(VRChatError("OSC manager not initialized".to_string())),
        }
    }

    pub async fn disconnect_osc(&self) -> Result<(), VRChatError> {
        match &self.osc_manager {
            Some(osc_manager) => {
                osc_manager.disconnect().await.map_err(|e| VRChatError(format!("Failed to disconnect OSC: {}", e)))
            },
            None => Err(VRChatError("OSC manager not initialized".to_string())),
        }
    }
}