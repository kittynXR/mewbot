use std::net::UdpSocket;
use rosc::{OscPacket, OscMessage, encoder, OscType};
use super::models::OSCMessage;

pub struct OSCClient {
    socket: UdpSocket,
    target_addr: String,
}

impl OSCClient {
    pub fn new(local_addr: &str, target_addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind(local_addr)?;
        Ok(Self {
            socket,
            target_addr: target_addr.to_string(),
        })
    }

    pub fn send_message(&self, message: &OSCMessage) -> std::io::Result<()> {
        let packet = OscPacket::Message(OscMessage {
            addr: message.address.clone(),
            args: message.args.clone(),
        });
        let encoded = encoder::encode(&packet).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        self.socket.send_to(&encoded, &self.target_addr)?;
        Ok(())
    }
}