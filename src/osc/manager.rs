use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

use crate::osc::client::OSCClient;
use crate::osc::errors::OSCError;
use crate::osc::models::{OSCMessageType, OSCValue};
use crate::osc::vrchat::VRChatOSC;

pub struct OSCManager {
    client: Arc<RwLock<OSCClient>>,
    vrchat_osc: Arc<VRChatOSC>,
    connection_status: Arc<RwLock<bool>>,
    last_reconnect_attempt: Arc<RwLock<Instant>>,
}

impl Default for OSCManager {
    fn default() -> Self {
        let client = Arc::new(RwLock::new(OSCClient::default()));
        Self {
            client: client.clone(),
            vrchat_osc: Arc::new(VRChatOSC::new(client)),
            connection_status: Arc::new(RwLock::new(false)),
            last_reconnect_attempt: Arc::new(RwLock::new(Instant::now())),
        }
    }
}

impl OSCManager {
    pub async fn new(target_addr: &str) -> Result<Self, OSCError> {
        let client = Arc::new(RwLock::new(OSCClient::new(target_addr).await?));
        let vrchat_osc = Arc::new(VRChatOSC::new(Arc::clone(&client)));

        Ok(Self {
            client,
            vrchat_osc,
            connection_status: Arc::new(RwLock::new(false)),
            last_reconnect_attempt: Arc::new(RwLock::new(Instant::now())),
        })
    }

    pub async fn connect(&self) -> Result<(), OSCError> {
        self.client.write().await.connect().await?;
        *self.connection_status.write().await = true;
        Ok(())
    }

    pub async fn disconnect(&self) -> Result<(), OSCError> {
        self.client.write().await.disconnect().await?;
        *self.connection_status.write().await = false;
        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        *self.connection_status.read().await
    }

    pub async fn reconnect(&self) -> Result<(), OSCError> {
        let mut last_reconnect = self.last_reconnect_attempt.write().await;
        if last_reconnect.elapsed() < Duration::from_secs(5) {
            return Err(OSCError::ReconnectTooSoon);
        }
        *last_reconnect = Instant::now();
        drop(last_reconnect);

        self.disconnect().await?;
        self.connect().await
    }

    pub async fn send_osc_message(&self, endpoint: &str, message_type: &OSCMessageType, value: &OSCValue) -> Result<(), OSCError> {
        if !self.is_connected().await {
            self.reconnect().await?;
        }
        self.client.read().await.send_osc_message(endpoint, message_type, value).await
    }

    pub fn get_vrchat_osc(&self) -> Arc<VRChatOSC> {
        Arc::clone(&self.vrchat_osc)
    }
}