use tokio::net::UdpSocket;
use std::net::SocketAddr;
use rosc::{OscPacket, OscMessage, OscType};

use crate::osc::errors::OSCError;
use crate::osc::models::{OSCMessageType, OSCValue};

pub struct OSCClient {
    socket: UdpSocket,
    target_addr: SocketAddr,
}

impl Default for OSCClient {
    fn default() -> Self {
        Self {
            socket: UdpSocket::from_std(std::net::UdpSocket::bind("0.0.0.0:0").unwrap()).unwrap(),
            target_addr: "127.0.0.1:9000".parse().unwrap(),
        }
    }
}

impl OSCClient {
    pub async fn new(target_addr: &str) -> Result<Self, OSCError> {
        let socket = UdpSocket::bind("0.0.0.0:0").await?;
        let target_addr: SocketAddr = target_addr.parse()?;

        Ok(Self {
            socket,
            target_addr,
        })
    }

    pub async fn connect(&mut self) -> Result<(), OSCError> {
        self.socket.connect(self.target_addr).await?;
        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), OSCError> {
        // For UDP, we don't need to explicitly disconnect.
        // This method is here for consistency and future-proofing.
        Ok(())
    }

    pub async fn send_osc_message(&self, endpoint: &str, message_type: &OSCMessageType, value: &OSCValue) -> Result<(), OSCError> {
        let osc_type = match (message_type, value) {
            (OSCMessageType::Boolean, OSCValue::Boolean(b)) => OscType::Bool(*b),
            (OSCMessageType::Integer, OSCValue::Integer(i)) => OscType::Int(*i),
            (OSCMessageType::Float, OSCValue::Float(f)) => OscType::Float(*f),
            (OSCMessageType::String, OSCValue::String(s)) => OscType::String(s.clone()),
            _ => return Err(OSCError::MismatchedType),
        };

        let packet = OscPacket::Message(OscMessage {
            addr: endpoint.to_string(),
            args: vec![osc_type],
        });

        let encoded = rosc::encoder::encode(&packet)?;
        self.socket.send(&encoded).await?;
        Ok(())
    }

    pub fn new_sync(target_addr: &str) -> Result<Self, OSCError> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        let target_addr: SocketAddr = target_addr.parse()?;

        Ok(Self {
            socket: UdpSocket::from_std(socket)?,
            target_addr,
        })
    }
}