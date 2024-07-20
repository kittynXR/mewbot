use super::client::OSCClient;
use super::models::OSCMessage;
use rosc::OscType;

pub struct VRChatOSC {
    client: OSCClient,
}

impl VRChatOSC {
    pub fn new(local_addr: &str, vrchat_addr: &str) -> std::io::Result<Self> {
        let client = OSCClient::new(local_addr, vrchat_addr)?;
        Ok(Self { client })
    }

    pub fn send_message(&self, address: &str, value: OscType) -> std::io::Result<()> {
        let message = OSCMessage {
            address: address.to_string(),
            args: vec![value],
        };
        self.client.send_message(&message)
    }
}