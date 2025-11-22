use anyhow::Result;
use iroh::endpoint::Endpoint;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::connection::logger::log_connection_details;
use crate::socks5::protocol::*;
use crate::tunnel::protocol::TunnelMessage;
use crate::tunnel::relay::{recv_message, send_message};
use crate::tunnel::state::{ConnectionState, TunnelState};
use crate::utils::logging::format_log;

pub async fn handle_socks_client(
    mut socket: TcpStream,
    _addr: SocketAddr,
    state: Arc<Mutex<TunnelState>>,
    endpoint: Endpoint,
) -> Result<()> {
    // SOCKS5 handshake
    let mut buf = [0u8; 2];
    socket.read_exact(&mut buf).await?;

    if buf[0] != SOCKS_VERSION {
        anyhow::bail!("Unsupported SOCKS version: {}", buf[0]);
    }

    let nmethods = buf[1] as usize;
    let mut methods = vec![0u8; nmethods];
    socket.read_exact(&mut methods).await?;

    // Reply: no authentication required
    socket.write_all(&[SOCKS_VERSION, 0]).await?;

    // Read request
    let mut buf = [0u8; 4];
    socket.read_exact(&mut buf).await?;

    if buf[0] != SOCKS_VERSION {
        anyhow::bail!("Invalid SOCKS version in request");
    }

    if buf[1] != SOCKS_CMD_CONNECT {
        // Send "command not supported"
        socket
            .write_all(&[SOCKS_VERSION, 7, 0, 1, 0, 0, 0, 0, 0, 0])
            .await?;
        anyhow::bail!("Only CONNECT command is supported");
    }

    // Parse destination address
    let (host, port) = match buf[3] {
        SOCKS_ADDR_TYPE_IPV4 => {
            let mut addr = [0u8; 4];
            socket.read_exact(&mut addr).await?;
            let mut port_buf = [0u8; 2];
            socket.read_exact(&mut port_buf).await?;
            let port = u16::from_be_bytes(port_buf);
            (
                format!("{}.{}.{}.{}", addr[0], addr[1], addr[2], addr[3]),
                port,
            )
        }
        SOCKS_ADDR_TYPE_DOMAIN => {
            let mut len = [0u8; 1];
            socket.read_exact(&mut len).await?;
            let mut domain = vec![0u8; len[0] as usize];
            socket.read_exact(&mut domain).await?;
            let mut port_buf = [0u8; 2];
            socket.read_exact(&mut port_buf).await?;
            let port = u16::from_be_bytes(port_buf);
            (String::from_utf8(domain)?, port)
        }
        SOCKS_ADDR_TYPE_IPV6 => {
            let mut addr = [0u8; 16];
            socket.read_exact(&mut addr).await?;
            let mut port_buf = [0u8; 2];
            socket.read_exact(&mut port_buf).await?;
            let port = u16::from_be_bytes(port_buf);
            // Format IPv6 address
            let ipv6_str = format!(
                "{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}:{:02x}{:02x}",
                addr[0],
                addr[1],
                addr[2],
                addr[3],
                addr[4],
                addr[5],
                addr[6],
                addr[7],
                addr[8],
                addr[9],
                addr[10],
                addr[11],
                addr[12],
                addr[13],
                addr[14],
                addr[15]
            );
            (format!("[{}]", ipv6_str), port)
        }
        _ => {
            socket
                .write_all(&[SOCKS_VERSION, 8, 0, 1, 0, 0, 0, 0, 0, 0])
                .await?;
            anyhow::bail!("Unsupported address type");
        }
    };

    println!("\n📥 {}", format_log("PROXY REQUEST", &host, port));

    // Get peer connection with wait-for-reconnection logic
    let peer_conn = {
        const MAX_WAIT: std::time::Duration = std::time::Duration::from_secs(5);
        const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(100);
        let start = std::time::Instant::now();

        loop {
            let (conn, conn_state) = {
                let state_guard = state.lock().await;
                (
                    state_guard.peer_connection.clone(),
                    state_guard.connection_state.clone(),
                )
            };

            if let Some(conn) = conn {
                break conn;
            }

            if conn_state == ConnectionState::Connecting {
                if start.elapsed() < MAX_WAIT {
                    tokio::time::sleep(CHECK_INTERVAL).await;
                    continue; // Wait for reconnection
                }
            }

            // No connection and not reconnecting, or timeout
            eprintln!("❌ No peer connection available (state: {:?})", conn_state);
            socket
                .write_all(&[SOCKS_VERSION, 4, 0, 1, 0, 0, 0, 0, 0, 0])
                .await?;
            anyhow::bail!("No peer connection");
        }
    };

    log_connection_details(&endpoint, peer_conn.remote_id(), "   ℹ️  Connection Info:");

    // Open tunnel stream
    let (mut send, mut recv) = peer_conn.open_bi().await?;

    // Send connect request
    send_message(
        &mut send,
        &TunnelMessage::Connect {
            host: host.clone(),
            port,
        },
    )
    .await?;

    // Wait for response
    match recv_message(&mut recv).await? {
        TunnelMessage::Connected => {
            println!("✅ {}", format_log("TUNNEL ESTABLISHED", &host, port));
            // Send success reply
            socket
                .write_all(&[SOCKS_VERSION, 0, 0, 1, 0, 0, 0, 0, 0, 0])
                .await?;

            // Relay data bidirectionally
            let (sent, received, sni) =
                crate::tunnel::relay::relay_bidirectional(&mut send, &mut recv, socket).await;
            println!(
                "   📊 Stats: ↑ {} bytes sent, ↓ {} bytes received{}",
                sent,
                received,
                sni.map(|s| format!(" (SNI: {})", s)).unwrap_or_default()
            );
        }
        TunnelMessage::Error { message } => {
            eprintln!("❌ Tunnel error: {}", message);
            socket
                .write_all(&[SOCKS_VERSION, 5, 0, 1, 0, 0, 0, 0, 0, 0])
                .await?;
            anyhow::bail!("Tunnel connection failed: {}", message);
        }
        _ => {
            socket
                .write_all(&[SOCKS_VERSION, 1, 0, 1, 0, 0, 0, 0, 0, 0])
                .await?;
            anyhow::bail!("Unexpected response");
        }
    }

    Ok(())
}
