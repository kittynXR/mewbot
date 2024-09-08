use thiserror::Error;

#[derive(Error, Debug)]
pub enum StateTransitionError {
    #[error("Invalid state transition")]
    InvalidTransition,
    #[error("Failed to acquire lock")]
    LockError,
    #[error("Failed to send state update")]
    BroadcastError,
}