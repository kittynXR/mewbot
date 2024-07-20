use rosc::{OscPacket, OscType};

pub struct OSCMessage {
    pub address: String,
    pub args: Vec<OscType>,
}