use std::collections::HashMap;
use std::sync::Arc;

use crate::osc::manager::OSCManager;
use crate::osc::errors::OSCError;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum EventType {
    ChatMessage,
    Redeem,
    EventSub,
}

#[derive(Debug, Clone)]
pub struct EventMessage {
    pub event_type: EventType,
    pub endpoint: String,
    pub message: String,
    pub user: Option<String>,
    pub redeem_title: Option<String>,
    pub event_data: Option<serde_json::Value>,
}

#[derive(Debug, Clone)]
pub struct EventConfig {
    pub event_type: EventType,
    pub osc_endpoint: String,
}

pub struct EventHandler {
    osc_manager: Arc<OSCManager>,
    event_configs: HashMap<EventType, EventConfig>,
}

impl EventHandler {
    pub fn new(osc_manager: Arc<OSCManager>) -> Self {
        Self {
            osc_manager,
            event_configs: HashMap::new(),
        }
    }

    pub fn add_event_config(&mut self, config: EventConfig) {
        self.event_configs.insert(config.event_type.clone(), config);
    }

    pub async fn process_event(&self, event: EventMessage) -> Result<(), OSCError> {
        if let Some(config) = self.event_configs.get(&event.event_type) {
            let vrchat_osc = self.osc_manager.get_vrchat_osc();
            match event.event_type {
                EventType::ChatMessage => {
                    vrchat_osc.send_chatbox_message(&event.message, true, true).await?;
                },
                EventType::Redeem => {
                    if let (Some(redeem_title), Some(user)) = (event.redeem_title, event.user) {
                        vrchat_osc.send_redeem_event(&redeem_title, &user).await?;
                    } else {
                        return Err(OSCError::MissingInfo);
                    }
                },
                EventType::EventSub => {
                    if let Some(event_data) = event.event_data {
                        vrchat_osc.send_eventsub_event(&event.endpoint, &event_data).await?;
                    } else {
                        return Err(OSCError::MissingInfo);
                    }
                },
            }
        }
        Ok(())
    }
}