use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OSCConfig {
    pub uses_osc: bool,
    pub osc_endpoint: String,
    pub osc_type: OSCMessageType,
    pub osc_value: OSCValue,
    pub default_value: OSCValue,
    #[serde(with = "duration_frames")]
    pub execution_duration: Option<u32>,
    pub send_chat_message: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OSCMessageType {
    Integer,
    Float,
    String,
    Boolean,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OSCValue {
    Integer(i32),
    Float(f32),
    String(String),
    Boolean(bool),
}

mod duration_frames {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(frames: &Option<u32>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match frames {
            Some(f) => serializer.serialize_u32(*f),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<u32>::deserialize(deserializer)
    }
}