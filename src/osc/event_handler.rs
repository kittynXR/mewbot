use super::vrchat::VRChatOSC;
use super::models::{EventMessage, EventConfig, EventType};
use std::collections::HashMap;
use std::sync::Arc;

pub struct EventHandler {
    vrchat_osc: Arc<VRChatOSC>,
    event_configs: HashMap<EventType, EventConfig>,
}

impl EventHandler {
    pub fn new(vrchat_osc: Arc<VRChatOSC>) -> Self {
        Self {
            vrchat_osc,
            event_configs: HashMap::new(),
        }
    }

    pub fn add_event_config(&mut self, config: EventConfig) {
        self.event_configs.insert(config.event_type.clone(), config);
    }

    pub fn process_event(&self, event: EventMessage) -> std::io::Result<()> {
        if let Some(config) = self.event_configs.get(&event.event_type) {
            match event.event_type {
                EventType::ChatMessage => {
                    self.vrchat_osc.send_chatbox_message(&event.message, true, true)
                },
                EventType::Redeem => {
                    if let (Some(redeem_title), Some(user)) = (event.redeem_title, event.user) {
                        self.vrchat_osc.send_redeem_event(&redeem_title, &user)
                    } else {
                        Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Missing redeem information"))
                    }
                },
                EventType::EventSub => {
                    if let Some(event_data) = event.event_data {
                        self.vrchat_osc.send_eventsub_event(&event.endpoint, &event_data)
                    } else {
                        Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Missing EventSub data"))
                    }
                },
            }
        } else {
            Ok(()) // Do nothing for unconfigured events
        }
    }
}