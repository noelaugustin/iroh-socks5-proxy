use anyhow::Result;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::http::parser::extract_http_info;
use crate::tls::sni::extract_sni;
use crate::tunnel::protocol::TunnelMessage;

pub async fn send_message(
    stream: &mut iroh::endpoint::SendStream,
    msg: &TunnelMessage,
) -> Result<()> {
    let data = bincode::serialize(msg)?;
    let len = (data.len() as u32).to_be_bytes();
    stream.write_all(&len).await?;
    stream.write_all(&data).await?;
    Ok(())
}

pub async fn recv_message(stream: &mut iroh::endpoint::RecvStream) -> Result<TunnelMessage> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;

    let msg = bincode::deserialize(&buf)?;
    Ok(msg)
}

// Relay data bidirectionally between tunnel streams and TCP socket
// Returns (bytes_sent, bytes_received, sni)
pub async fn relay_bidirectional(
    send: &mut iroh::endpoint::SendStream,
    recv: &mut iroh::endpoint::RecvStream,
    mut socket: TcpStream,
) -> (u64, u64, Option<String>) {
    // We can't use tokio::spawn with borrowed data, so we do manual bidirectional relay
    let (mut socket_read, mut socket_write) = socket.split();

    let mut send_buf = vec![0u8; 8192];
    let mut first_packet_socket = true;
    let mut first_packet_tunnel = true;
    let mut sni = None;
    let mut bytes_sent = 0u64;
    let mut bytes_received = 0u64;

    loop {
        tokio::select! {
            // Read from socket, write to tunnel
            result = socket_read.read(&mut send_buf) => {
                match result {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        // Try to extract protocol info from first packet FROM socket
                        if first_packet_socket {
                            first_packet_socket = false;

                            // Try TLS SNI first
                            if let Some(extracted_sni) = extract_sni(&send_buf[..n]) {
                                sni = Some(extracted_sni.clone());
                                println!("   🔒 SNI: {}", extracted_sni);
                            }
                            // If not TLS, try HTTP
                            else if let Some(http_info) = extract_http_info(&send_buf[..n]) {
                                let host_display = http_info.host.as_deref().unwrap_or("unknown");
                                sni = Some(format!("{} {}", http_info.method, http_info.path));
                                println!("   🌐 HTTP: {} {} (Host: {})",
                                    http_info.method,
                                    http_info.path,
                                    host_display
                                );
                            }
                        }

                        bytes_sent += n as u64;
                        let msg = TunnelMessage::Data {
                            data: send_buf[..n].to_vec(),
                        };
                        if send_message(send, &msg).await.is_err() {
                            break;
                        }
                    }
                }
            }
            // Read from tunnel, write to socket
            result = recv_message(recv) => {
                match result {
                    Ok(TunnelMessage::Data { data }) => {
                        // Try to extract protocol info from first packet FROM tunnel
                        if first_packet_tunnel {
                            first_packet_tunnel = false;

                            // Try TLS SNI first
                            if let Some(extracted_sni) = extract_sni(&data) {
                                sni = Some(extracted_sni.clone());
                                println!("   🔒 SNI: {}", extracted_sni);
                            }
                            // If not TLS, try HTTP
                            else if let Some(http_info) = extract_http_info(&data) {
                                let host_display = http_info.host.as_deref().unwrap_or("unknown");
                                sni = Some(format!("{} {}", http_info.method, http_info.path));
                                println!("   🌐 HTTP: {} {} (Host: {})",
                                    http_info.method,
                                    http_info.path,
                                    host_display
                                );
                            }
                        }

                        bytes_received += data.len() as u64;
                        if socket_write.write_all(&data).await.is_err() {
                            break;
                        }
                    }
                    Ok(TunnelMessage::Close) | Err(_) => break,
                    _ => {}
                }
            }
        }
    }

    send_message(send, &TunnelMessage::Close).await.ok();
    (bytes_sent, bytes_received, sni)
}
