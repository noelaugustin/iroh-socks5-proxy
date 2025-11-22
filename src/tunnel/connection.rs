use anyhow::{Context, Result};
use iroh::endpoint::{Connection, Endpoint};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;

use crate::connection::logger::log_connection_details;
use crate::socks5::protocol::is_loopback_address;
use crate::tunnel::protocol::TunnelMessage;
use crate::tunnel::relay::{recv_message, relay_bidirectional, send_message};
use crate::tunnel::state::{ConnectionState, TUNNEL_ALPN, TunnelState};
use crate::utils::logging::format_log;

pub async fn monitor_connection_health(state: Arc<Mutex<TunnelState>>, endpoint: Endpoint) {
    const HEALTH_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

    loop {
        tokio::time::sleep(HEALTH_CHECK_INTERVAL).await;

        let (should_reconnect, remote_peer_id) = {
            let mut state = state.lock().await;
            match &state.peer_connection {
                Some(conn) if conn.close_reason().is_some() => {
                    // Connection is closed
                    eprintln!("⚠️  Connection lost, will attempt reconnection...");
                    state.connection_state = ConnectionState::Disconnected;
                    state.peer_connection = None;
                    (true, state.remote_peer_id)
                }
                None if state.remote_peer_id.is_some() => {
                    // No connection but we know the peer - try to reconnect
                    (true, state.remote_peer_id)
                }
                _ => (false, None),
            }
        };

        if should_reconnect {
            if let Some(peer_id) = remote_peer_id {
                attempt_reconnection(&state, &endpoint, peer_id).await;
            }
        }
    }
}

pub async fn attempt_reconnection(
    state: &Arc<Mutex<TunnelState>>,
    endpoint: &Endpoint,
    remote_peer_id: iroh::PublicKey,
) {
    const BASE_DELAY: std::time::Duration = std::time::Duration::from_secs(1);
    const MAX_DELAY: std::time::Duration = std::time::Duration::from_secs(60);

    let attempts = {
        let state = state.lock().await;
        state.reconnect_attempts
    };

    // Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s, 60s (max)
    let delay = BASE_DELAY * 2_u32.pow(attempts.min(6));
    let delay = delay.min(MAX_DELAY);

    println!(
        "🔄 Reconnection attempt #{} to {} in {:?}...",
        attempts + 1,
        remote_peer_id,
        delay
    );
    tokio::time::sleep(delay).await;

    {
        let mut state = state.lock().await;
        state.connection_state = ConnectionState::Connecting;
        state.reconnect_attempts += 1;
    }

    // Connect directly using PublicKey (works for both client and server)
    match endpoint.connect(remote_peer_id, TUNNEL_ALPN).await {
        Ok(conn) => {
            println!("✅ Reconnected to peer: {}", conn.remote_id());

            // Update state
            {
                let mut state_guard = state.lock().await;
                state_guard.peer_connection = Some(conn.clone());
                state_guard.connection_state = ConnectionState::Connected;
                state_guard.reconnect_attempts = 0; // Reset on success
            }

            // Spawn new handler
            let endpoint_clone = endpoint.clone();
            let state_clone = Arc::clone(state);
            tokio::spawn(async move {
                handle_peer_connection_with_monitoring(conn, endpoint_clone, state_clone).await;
            });
        }
        Err(e) => {
            eprintln!("❌ Reconnection failed: {}", e);
            let mut state = state.lock().await;
            state.connection_state = ConnectionState::Failed;
        }
    }
}

pub async fn handle_peer_connection_with_monitoring(
    connection: Connection,
    endpoint: Endpoint,
    state: Arc<Mutex<TunnelState>>,
) {
    handle_peer_connection(connection.clone(), endpoint).await;

    // When handler exits, clear the connection
    let mut state_lock = state.lock().await;
    if let Some(conn) = &state_lock.peer_connection {
        if conn.stable_id() == connection.stable_id() {
            eprintln!("⚠️  Peer connection handler exited");
            state_lock.peer_connection = None;
            state_lock.connection_state = ConnectionState::Disconnected;
        }
    }
}

pub async fn connect_to_peer(endpoint: &Endpoint, ticket: &str) -> Result<Connection> {
    // Parse the ticket as a PublicKey (NodeId)
    let public_key: iroh::PublicKey = ticket
        .parse()
        .context("Failed to parse PublicKey from ticket")?;

    println!("🔌 Attempting to connect to node: {}", public_key);

    // Connect to the peer using the PublicKey
    // Iroh will use its discovery mechanisms to find the peer
    let connection = endpoint
        .connect(public_key, TUNNEL_ALPN)
        .await
        .context("Failed to connect to peer")?;

    println!("✅ Successfully connected to peer!");

    Ok(connection)
}

pub async fn generate_ticket(endpoint: &Endpoint) -> Result<String> {
    // Generate a simple ticket with node ID
    let node_id = endpoint.id();
    Ok(format!("{}", node_id))
}

pub async fn handle_peer_connection(connection: Connection, endpoint: Endpoint) {
    let remote_node_id = connection.remote_id();
    let endpoint_clone = endpoint.clone();

    // Handle incoming tunnel requests from peer
    loop {
        match connection.accept_bi().await {
            Ok((mut send, mut recv)) => {
                let endpoint = endpoint_clone.clone();
                tokio::spawn(async move {
                    if let Err(e) =
                        handle_tunnel_request(&mut send, &mut recv, endpoint, remote_node_id).await
                    {
                        eprintln!("❌ Tunnel request error: {}", e);
                    }
                });
            }
            Err(e) => {
                eprintln!("❌ Failed to accept bi-stream: {}", e);
                break;
            }
        }
    }
}

pub async fn handle_tunnel_request(
    send: &mut iroh::endpoint::SendStream,
    recv: &mut iroh::endpoint::RecvStream,
    endpoint: Endpoint,
    remote_node_id: iroh::PublicKey,
) -> Result<()> {
    // Read the connect message
    let msg = recv_message(recv).await?;

    match msg {
        TunnelMessage::Connect { host, port } => {
            let log_prefix = format!("\n📤 {}", format_log("OUTGOING", &host, port));
            println!("{}", log_prefix);
            log_connection_details(&endpoint, remote_node_id, "   ℹ️  Connection Info:");

            // LOOP PREVENTION: Check if the destination is localhost on our SOCKS port
            if is_loopback_address(&host, port) {
                eprintln!(
                    "⚠️  Loop detected! Rejecting connection to {}:{}",
                    host, port
                );
                send_message(
                    send,
                    &TunnelMessage::Error {
                        message: "Loop detected: cannot tunnel to local SOCKS proxy".to_string(),
                    },
                )
                .await?;
                return Ok(());
            }

            // Connect to the actual destination
            match TcpStream::connect(format!("{}:{}", host, port)).await {
                Ok(remote) => {
                    println!("✅ {}", format_log("CONNECTED", &host, port));
                    send_message(send, &TunnelMessage::Connected).await?;

                    // Relay data bidirectionally
                    let (sent, received, sni) = relay_bidirectional(send, recv, remote).await;
                    println!(
                        "   📊 Stats: ↑ {} bytes sent, ↓ {} bytes received{}",
                        sent,
                        received,
                        sni.map(|s| format!(" (SNI: {})", s)).unwrap_or_default()
                    );
                }
                Err(e) => {
                    eprintln!("❌ Failed to connect to {}:{}: {}", host, port, e);
                    send_message(
                        send,
                        &TunnelMessage::Error {
                            message: format!("Connection failed: {}", e),
                        },
                    )
                    .await?;
                }
            }
        }
        _ => {
            eprintln!("❌ Unexpected message type");
        }
    }

    Ok(())
}
