use std::time::Duration;
use rosc::{OscPacket, OscType};
use serde::{Deserialize, Serialize};

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


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OSCConfig {
    pub uses_osc: bool,
    pub osc_endpoint: String,
    pub osc_type: OSCMessageType,
    pub osc_value: OSCValue,
    pub default_value: OSCValue,
    pub execution_duration: Option<Duration>,
    pub send_chat_message: bool,  // New field
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OSCMessageType {
    Boolean,
    Integer,
    Float,
    String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OSCValue {
    Boolean(bool),
    Integer(i32),
    Float(f32),
    String(String),
}