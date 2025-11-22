use iroh::endpoint::Connection;

pub const TUNNEL_ALPN: &[u8] = b"iroh-tunnel/1";

#[derive(Clone, Debug, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

pub struct TunnelState {
    pub peer_connection: Option<Connection>,
    pub connection_state: ConnectionState,
    pub remote_peer_id: Option<iroh::PublicKey>,
    pub reconnect_attempts: u32,
    pub last_connection_attempt: Option<std::time::Instant>,
    pub _log_file: Option<String>,
}
