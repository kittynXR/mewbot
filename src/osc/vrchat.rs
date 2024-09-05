use std::sync::Arc;

use crate::osc::client::OSCClient;
use crate::osc::errors::OSCError;
use crate::osc::models::{OSCConfig, OSCMessageType, OSCValue};

pub struct VRChatOSC {
    client: Arc<tokio::sync::RwLock<OSCClient>>,
}

impl VRChatOSC {
    pub fn new(client: Arc<tokio::sync::RwLock<OSCClient>>) -> Self {
        Self { client }
    }

    pub async fn send_chatbox_message(&self, message: &str, send_immediately: bool, play_sound: bool) -> Result<(), OSCError> {
        let truncated_message = message.chars().take(144).collect::<String>();

        self.client.read().await.send_osc_message(
            "/chatbox/input",
            &OSCMessageType::String,
            &OSCValue::String(truncated_message),
        ).await?;

        self.client.read().await.send_osc_message(
            "/chatbox/typing",
            &OSCMessageType::Boolean,
            &OSCValue::Boolean(send_immediately),
        ).await?;

        if play_sound {
            self.client.read().await.send_osc_message(
                "/chatbox/audio",
                &OSCMessageType::Boolean,
                &OSCValue::Boolean(true),
            ).await?;
        }

        Ok(())
    }

    pub async fn send_redeem_event(&self, redeem_title: &str) -> Result<(), OSCError> {
        self.client.read().await.send_osc_message(
            "/avatar/parameters/LastRedeem",
            &OSCMessageType::String,
            &OSCValue::String(redeem_title.to_string()),
        ).await
    }

    pub async fn send_eventsub_event(&self, event_type: &str, _data: &serde_json::Value) -> Result<(), OSCError> {
        self.client.read().await.send_osc_message(
            "/avatar/parameters/LastEventSub",
            &OSCMessageType::String,
            &OSCValue::String(event_type.to_string()),
        ).await
    }

    pub async fn send_osc_message_with_reset(&self, config: &OSCConfig) -> Result<(), OSCError> {
        self.client.read().await.send_osc_message(&config.osc_endpoint, &config.osc_type, &config.osc_value).await?;

        if let Some(duration) = config.execution_duration {
            tokio::time::sleep(duration).await;
            self.client.read().await.send_osc_message(&config.osc_endpoint, &config.osc_type, &config.default_value).await?;
        }

        Ok(())
    }
}