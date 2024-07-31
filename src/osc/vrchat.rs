use std::net::UdpSocket;
use rosc::{OscPacket, OscMessage, OscType};
use super::models::{EventType, EventMessage};

pub struct VRChatOSC {
    socket: UdpSocket,
    target_addr: String,
}

impl VRChatOSC {
    pub fn new(target_addr: &str) -> std::io::Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.connect(target_addr)?;

        Ok(Self {
            socket,
            target_addr: target_addr.to_string(),
        })
    }

    pub fn send_chatbox_message(&self, message: &str, send_immediately: bool, play_sound: bool) -> std::io::Result<()> {
        let truncated_message = message.chars().take(144).collect::<String>();

        let packet = OscPacket::Message(OscMessage {
            addr: "/chatbox/input".to_string(),
            args: vec![
                OscType::String(truncated_message.clone()),
                OscType::Bool(send_immediately),
                OscType::Bool(play_sound),
            ],
        });

        self.send_osc_packet(packet, &truncated_message)
    }

    pub fn send_redeem_event(&self, redeem_title: &str, user: &str) -> std::io::Result<()> {
        let message = format!("{} redeemed {}", user, redeem_title);
        let packet = OscPacket::Message(OscMessage {
            addr: "/avatar/parameters/LastRedeem".to_string(),
            args: vec![OscType::String(redeem_title.to_string())],
        });

        self.send_osc_packet(packet, &message)
    }

    pub fn send_eventsub_event(&self, event_type: &str, data: &serde_json::Value) -> std::io::Result<()> {
        let message = format!("EventSub: {} occurred", event_type);
        let packet = OscPacket::Message(OscMessage {
            addr: "/avatar/parameters/LastEventSub".to_string(),
            args: vec![OscType::String(event_type.to_string())],
        });

        self.send_osc_packet(packet, &message)
    }

    fn send_osc_packet(&self, packet: OscPacket, message: &str) -> std::io::Result<()> {
        let encoded = rosc::encoder::encode(&packet)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        match self.socket.send(&encoded) {
            Ok(bytes_sent) => {
                println!("OSC message sent successfully. Bytes sent: {}", bytes_sent);
                println!("Message content: {}", message);
                Ok(())
            },
            Err(e) => {
                eprintln!("Failed to send OSC message: {}", e);
                Err(e)
            }
        }
    }
}