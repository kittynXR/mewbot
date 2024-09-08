mod errors;
mod manager;

pub use errors::StateTransitionError;
pub use manager::{StreamState, StreamStateMachine};