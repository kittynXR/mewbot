mod manager;
pub(crate) mod client;
mod vrchat;
pub(crate) mod models;
pub(crate) mod osc_config;
mod errors;

pub use manager::OSCManager;
pub use vrchat::VRChatOSC;
pub use models::{OSCConfig, OSCMessageType, OSCValue};
pub use osc_config::OSCConfigurations;
pub use errors::OSCError;