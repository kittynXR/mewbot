// src/obs/mod.rs

mod models;
mod websocket;

pub use models::{
    OBSInstance,
    OBSScene,
    OBSSceneItem,
    OBSWebSocketClient,
    OBSClientState,
    OBSInstanceState,
    OBSManager,
    OBSStateUpdate,
};

pub use websocket::{
    OBSResponse,
    HelloMessage,
    AuthenticationInfo,
};

// Constants
pub use websocket::{
    TIMEOUT_DURATION,
    MAX_RECONNECT_DELAY,
};

// If there are any functions or constants in the websocket.rs file that need to be public,
// you can re-export them here as well.