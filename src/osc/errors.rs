use thiserror::Error;
use rosc::OscError;

#[derive(Error, Debug)]
pub enum OSCError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Address parse error: {0}")]
    AddrParse(#[from] std::net::AddrParseError),

    #[error("OSC encode error: {0}")]
    OscEncode(#[from] OscError),

    #[error("Mismatched OSC type and value")]
    MismatchedType,

    #[error("Missing required information")]
    MissingInfo,

    #[error("Reconnection attempted too soon")]
    ReconnectTooSoon,

    #[error("OSC client is not connected")]
    NotConnected,

    #[error("Unknown OSC error: {0}")]
    Unknown(String),
}

// Remove the manual implementation of From<OscError>