use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::Duration;
use crate::osc::models::{OSCConfig, OSCMessageType, OSCValue};

#[derive(Serialize, Deserialize)]
pub struct OSCConfigurations {
    pub configs: HashMap<String, OSCConfig>,
}

impl OSCConfigurations {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let configs: OSCConfigurations = serde_json::from_str(&content)?;
            Ok(configs)
        } else {
            let default_configs = Self::default();
            default_configs.save(path)?;
            Ok(default_configs)
        }
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn get_config(&self, key: &str) -> Option<&OSCConfig> {
        self.configs.get(key)
    }

    pub fn add_config(&mut self, key: &str, config: OSCConfig) {
        self.configs.insert(key.to_string(), config);
    }
}

impl Default for OSCConfigurations {
    fn default() -> Self {
        let mut configs = HashMap::new();
        configs.insert("channel.follow".to_string(), OSCConfig {
            uses_osc: true,
            osc_endpoint: "/avatar/parameters/NewFollower".to_string(),
            osc_type: OSCMessageType::Boolean,
            osc_value: OSCValue::Boolean(true),
            default_value: OSCValue::Boolean(false),
            execution_duration: Some(Duration::from_secs(5)),
            send_chat_message: false,
        });
        configs.insert("channel.subscribe".to_string(), OSCConfig {
            uses_osc: true,
            osc_endpoint: "/avatar/parameters/NewSubscriber".to_string(),
            osc_type: OSCMessageType::Boolean,
            osc_value: OSCValue::Boolean(true),
            default_value: OSCValue::Boolean(false),
            execution_duration: Some(Duration::from_secs(5)),
            send_chat_message: false,
        });
        OSCConfigurations { configs }
    }
}