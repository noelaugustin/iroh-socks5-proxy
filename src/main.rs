use anyhow::{Context, Result};
use clap::Parser;
use iroh::endpoint::Endpoint;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use iroh_socks5_proxy::tunnel::connection::{
    generate_ticket, handle_peer_connection_with_monitoring, monitor_connection_health,
};
use iroh_socks5_proxy::tunnel::persistence::{
    clear_remote_peer_id, get_or_create_secret_key, save_remote_peer_id,
};
use iroh_socks5_proxy::tunnel::socks::handle_socks_client;
use iroh_socks5_proxy::tunnel::state::{ConnectionState, TUNNEL_ALPN, TunnelState};

#[derive(Parser, Debug)]
#[command(author, version, about = "Iroh-based SOCKS5 tunnel", long_about = None)]
struct Args {
    /// Local SOCKS5 proxy port
    #[arg(short, long, default_value = "1080")]
    port: u16,

    /// Peer node ticket to connect to (optional, for client mode)
    #[arg(short = 'c', long)]
    peer: Option<String>,

    /// Log file path for request logging (optional)
    #[arg(short = 'l', long)]
    log_file: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    println!("🚇 Starting Iroh Tunnel...");

    // Determine if we should persist the secret key:
    // - Server mode (no -c flag): persist key for stable Node ID
    // - Client mode (with -c flag): ephemeral key for random Node ID each run
    let persist_key = args.peer.is_none();

    let secret_key = get_or_create_secret_key(persist_key).await?;

    // Setup Iroh Endpoint
    let endpoint = Endpoint::builder()
        .secret_key(secret_key)
        .alpns(vec![TUNNEL_ALPN.to_vec()])
        .bind()
        .await
        .context("Failed to bind Iroh endpoint")?;

    println!("📡 Node ID: {}", endpoint.id());
    println!("🔗 Endpoints: (waiting for discovery...)");
    println!();

    // Determine which peer to connect to based on command line args
    let peer_to_connect = if let Some(peer_ticket) = &args.peer {
        // Client mode: explicit peer provided via -c flag
        // Clear any old persisted peer to ensure fresh connection
        clear_remote_peer_id().await.ok();

        let peer_id: iroh::PublicKey =
            peer_ticket.parse().context("Failed to parse peer ticket")?;

        Some(peer_id)
    } else {
        // Server mode: don't load persisted peer, just wait for incoming connections
        // Clear any stale .tunnel_peer file to avoid confusion
        clear_remote_peer_id().await.ok();
        None
    };

    let state = Arc::new(Mutex::new(TunnelState {
        peer_connection: None,
        connection_state: ConnectionState::Disconnected,
        remote_peer_id: peer_to_connect,
        reconnect_attempts: 0,
        last_connection_attempt: None,
        _log_file: args.log_file.clone(),
    }));

    // If we have a peer to connect to (either from -c flag or persisted), connect to it (client mode)
    if let Some(peer_id) = peer_to_connect {
        // Persist to disk (only if explicitly provided via -c flag)
        if args.peer.is_some() {
            save_remote_peer_id(peer_id).await.ok();
        }

        println!("🔌 Connecting to peer...");
        match endpoint.connect(peer_id, TUNNEL_ALPN).await {
            Ok(conn) => {
                println!("✅ Connected to peer: {}", conn.remote_id());

                // Update state
                {
                    let mut state_guard = state.lock().await;
                    state_guard.peer_connection = Some(conn.clone());
                    state_guard.connection_state = ConnectionState::Connected;
                }

                // Spawn handler with monitoring
                let endpoint_clone = endpoint.clone();
                let state_clone = Arc::clone(&state);
                tokio::spawn(async move {
                    handle_peer_connection_with_monitoring(conn, endpoint_clone, state_clone).await;
                });
            }
            Err(e) => {
                eprintln!("❌ Failed to connect to peer: {}", e);
                eprintln!("💡 Will keep retrying in background...");
                let mut state_guard = state.lock().await;
                state_guard.connection_state = ConnectionState::Failed;
            }
        }
    } else {
        println!("📋 Connection ticket (share this with peer):");
        println!("   {}", generate_ticket(&endpoint).await?);
        println!();
        println!("💡 Waiting for peer to connect...");
    }

    // Start connection health monitor for BOTH client and server modes
    {
        let state_clone = Arc::clone(&state);
        let endpoint_clone = endpoint.clone();
        tokio::spawn(async move {
            monitor_connection_health(state_clone, endpoint_clone).await;
        });
    }

    // Start SOCKS5 proxy server
    let socks_addr = format!("127.0.0.1:{}", args.port);
    let listener = TcpListener::bind(&socks_addr)
        .await
        .context("Failed to bind SOCKS5 server")?;

    println!("🌐 SOCKS5 proxy listening on {}", socks_addr);
    println!(
        "📝 Configure your browser/app to use SOCKS5 proxy: localhost:{}",
        args.port
    );
    println!();

    let state_clone = state.clone();
    let endpoint_clone = endpoint.clone();

    // Accept incoming Iroh connections
    tokio::spawn(async move {
        while let Some(incoming) = endpoint_clone.accept().await {
            let state_clone_inner = state_clone.clone();
            let endpoint_clone_inner = endpoint_clone.clone();
            match incoming.accept() {
                Ok(connecting) => {
                    tokio::spawn(async move {
                        match connecting.await {
                            Ok(connection) => {
                                let remote_id = connection.remote_id();
                                println!("✅ Peer connected: {}", remote_id);

                                // Store remote peer ID in memory only (server doesn't persist)
                                {
                                    let mut state_guard = state_clone_inner.lock().await;
                                    state_guard.peer_connection = Some(connection.clone());
                                    state_guard.remote_peer_id = Some(remote_id);
                                    state_guard.connection_state = ConnectionState::Connected;
                                }
                                // Note: Server mode does NOT persist peer ID to disk
                                // This allows accepting connections from any peer

                                // Spawn handler with monitoring
                                handle_peer_connection_with_monitoring(
                                    connection,
                                    endpoint_clone_inner,
                                    state_clone_inner,
                                )
                                .await;
                            }
                            Err(e) => eprintln!("❌ Connection error: {}", e),
                        }
                    });
                }
                Err(e) => eprintln!("❌ Failed to accept connection: {}", e),
            }
        }
    });

    // Accept SOCKS5 connections
    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                let state = state.clone();
                let endpoint = endpoint.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_socks_client(socket, addr, state, endpoint).await {
                        eprintln!("❌ SOCKS error from {}: {}", addr, e);
                    }
                });
            }
            Err(e) => eprintln!("❌ Failed to accept SOCKS connection: {}", e),
        }
    }
}
