// Tunnel protocol - TunnelMessage
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub enum TunnelMessage {
    Connect { host: String, port: u16 },
    Connected,
    Error { message: String },
    Data { data: Vec<u8> },
    Close,
}
