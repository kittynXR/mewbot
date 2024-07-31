use rosc::{OscPacket, OscType};

pub struct OSCMessage {
    pub address: String,
    pub args: Vec<OscType>,
}

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