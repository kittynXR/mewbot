pub mod ai_response;
mod osc_message;
mod custom_action;

pub use ai_response::AIResponseManager;
pub use ai_response::AIResponseConfig;
pub use ai_response::AIProvider;
pub use ai_response::AIResponseType;
pub use osc_message::handle_osc_message;
pub use custom_action::handle_custom_action;
pub use custom_action::handle_coin_game;  // Updated from handle_gamba_time