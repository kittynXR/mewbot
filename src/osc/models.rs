use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OSCConfig {
    pub uses_osc: bool,
    pub osc_endpoint: String,
    pub osc_type: OSCMessageType,
    pub osc_value: OSCValue,
    pub default_value: OSCValue,
    pub execution_duration: Option<Duration>,
    pub send_chat_message: bool,
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