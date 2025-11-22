# Iroh SOCKS5 Proxy

**Secure peer-to-peer SOCKS5 proxy over Iroh** - Route your internet traffic through a remote peer using encrypted, NAT-traversing connections.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)

## Overview

Iroh SOCKS5 Proxy creates a secure tunnel between two peers, allowing you to route internet traffic through a remote machine. Built on [Iroh](https://github.com/n0-computer/iroh), it leverages QUIC for encrypted, hole-punched connections that work across NATs and firewalls.

### Key Features

✅ **Automatic Reconnection** - Survives network interruptions and peer restarts  
✅ **Bidirectional** - Both peers can act as exit nodes simultaneously  
✅ **IPv4 & IPv6 Support** - Full dual-stack support for both protocols  
✅ **NAT Traversal** - Works across firewalls using hole-punching  
✅ **HTTP/HTTPS Logging** - Visibility into tunneled requests  
✅ **Loop Prevention** - Automatic detection of routing loops  
✅ **Zero Configuration** - No root privileges or external tools required  
✅ **Persistent Identity** - Node IDs survive restarts  
✅ **Production Ready** - Exponential backoff, health monitoring, graceful degradation

---

## Table of Contents

- [How It Works](#how-it-works)
- [Quick Start](#quick-start)
- [Command Line Reference](#command-line-reference)
- [Advanced Features](#advanced-features)
- [Network Architecture](#network-architecture)
- [Configuration Examples](#configuration-examples)
- [Troubleshooting](#troubleshooting)
- [Security & Privacy](#security--privacy)
- [Development](#development)

---

## How It Works

### Basic Concept

```
┌─────────────────┐                    ┌─────────────────┐
│   Local Peer    │                    │   Remote Peer   │
│   (Client)      │                    │   (Exit Node)   │
├─────────────────┤                    ├─────────────────┤
│                 │                    │                 │
│  Browser/App    │                    │                 │
│      ↓          │                    │                 │
│  SOCKS5 Proxy   │  ←Iroh Tunnel→     │  TCP Connect    │
│  (localhost)    │  (Encrypted QUIC)  │  (Internet)     │
│                 │                    │      ↓          │
│                 │                    │  google.com     │
└─────────────────┘                    └─────────────────┘
```

**Traffic Flow:**
1. Your browser connects to local SOCKS5 proxy (`localhost:1080`)
2. SOCKS5 proxy forwards request through encrypted Iroh tunnel
3. Remote peer receives request and connects to destination
4. Data flows bidirectionally through the tunnel
5. Browser receives response as if connecting directly

**Result:** Your traffic appears to originate from the remote peer's IP address.

### Protocol Stack

```
┌──────────────────────────────────────┐
│     Application (HTTP, HTTPS, etc)   │
├──────────────────────────────────────┤
│     SOCKS5 Protocol                  │
├──────────────────────────────────────┤
│     Tunnel Protocol (Custom)         │
├──────────────────────────────────────┤
│     Iroh (QUIC + Noise encryption)   │
├──────────────────────────────────────┤
│     UDP (with NAT traversal)         │
└──────────────────────────────────────┘
```

---

## Quick Start

### Installation

```bash
# Clone the repository
git clone https://github.com/noelaugustin/iroh-socks5-proxy.git
cd iroh-socks5-proxy

# Build the project
cargo build --release --bin tunnel
```

### Basic Usage

#### 1. Start the Exit Node (Server)

On the machine you want traffic to exit from:

```bash
./target/release/tunnel
```

**Expected Output:**
```
🚇 Starting Iroh Tunnel...
🔑 Loaded persistent secret key
📡 Node ID: 5j7k8m9nbvcxzaqwertyuiop...
🔗 Endpoints: (waiting for discovery...)

📋 Connection ticket (share this with peer):
   5j7k8m9nbvcxzaqwertyuiop1234567890abcdef...

💡 Waiting for peer to connect...
🌐 SOCKS5 proxy listening on 127.0.0.1:1080
📝 Configure your browser/app to use SOCKS5 proxy: localhost:1080
```

**Copy the connection ticket** - you'll give this to the client peer.

#### 2. Start the Client Peer

On your local machine:

```bash
./target/release/tunnel --peer "<ticket-from-server>"
```

**Expected Output:**
```
🚇 Starting Iroh Tunnel...
🔑 Loaded persistent secret key
📡 Node ID: 9x8y7z6w5v4u3t2s1r0q...
🔗 Endpoints: (waiting for discovery...)

🔌 Connecting to peer...
✅ Connected to peer: 5j7k8m9nbvcxzaqwertyuiop...
   ℹ️  Connection Info:
   📍 Remote Address: 203.0.113.42:54321
   🔌 Connection Type: direct (holepunched)
   ⏱️  Latency: 45ms

🌐 SOCKS5 proxy listening on 127.0.0.1:1080
📝 Configure your browser/app to use SOCKS5 proxy: localhost:1080
```

#### 3. Configure Your Application

**Firefox:**
1. Settings → Network Settings → Manual proxy configuration
2. SOCKS Host: `localhost`, Port: `1080`
3. SOCKS v5: ✓
4. Proxy DNS when using SOCKS v5: ✓

**Chrome/Chromium:**
```bash
chromium --proxy-server="socks5://localhost:1080"
```

**curl:**
```bash
curl --socks5 localhost:1080 https://ifconfig.me
```

#### 4. Verify

Visit [https://ifconfig.me](https://ifconfig.me) - you should see the **remote peer's IP address**.

---

## Command Line Reference

```
Usage: tunnel [OPTIONS]

Options:
  -p, --port <PORT>          Local SOCKS5 proxy port [default: 1080]
  -c, --peer <TICKET>        Peer connection ticket (client mode)
  -l, --log-file <PATH>      Request log file path (optional)
  -h, --help                 Print help
  -V, --version              Print version
```

### Examples

**Custom port:**
```bash
tunnel --port 9050
```

**Connect to specific peer:**
```bash
tunnel --peer "5j7k8m9nbvcxzaqwertyuiop..."
```

**Enable request logging:**
```bash
tunnel --log-file /var/log/proxy-requests.log
```

**Combined:**
```bash
tunnel --peer "..." --port 9050 --log-file proxy.log
```

---

## Advanced Features

### Automatic Reconnection

The tunnel automatically handles connection failures with intelligent retry logic:

- **Exponential Backoff:** 1s → 2s → 4s → 8s → 16s → 32s → 60s (max)
- **Bidirectional:** Both peers can initiate reconnection
- **Persistent State:** Connection survives peer restarts
- **Graceful Degradation:** SOCKS requests wait up to 5s for reconnection

**Technical Details:**
- Background health monitor checks connection every 5 seconds
- Connection state persisted to `.tunnel_peer` file
- Infinite retry ensures maximum uptime
- No manual intervention required

**User Experience:**
```
# Server restarts unexpectedly
Client: 🔄 Connection lost, attempting reconnection (attempt 1, delay 1s)...
Client: 🔄 Connection lost, attempting reconnection (attempt 2, delay 2s)...
# Server comes back online
Client: ✅ Reconnected to peer: 5j7k8m9n...
# Traffic resumes automatically
```

### HTTP/HTTPS Request Logging

See what's being tunneled in real-time:

**HTTPS (TLS SNI):**
```
📥 PROXY REQUEST: github.com:443
   ℹ️  Connection Info:
   📍 Remote Address: 203.0.113.42:54321
   🔌 Connection Type: relay
   ⏱️  Latency: 67ms
   🔒 SNI: github.com
✅ CONNECTED: github.com:443
   📊 Stats: ↑ 2,847 bytes sent, ↓ 15,392 bytes received (SNI: github.com)
```

**HTTP (Request Headers):**
```
📥 PROXY REQUEST: example.com:80
   ℹ️  Connection Info:
   📍 Remote Address: 203.0.113.42:54321
   🔌 Connection Type: direct (holepunched)
   ⏱️  Latency: 23ms
   🌐 HTTP: GET / (Host: example.com)
✅ CONNECTED: example.com:80
   📊 Stats: ↑ 1,234 bytes sent, ↓ 5,678 bytes received (GET /)
```

**Logged on both peers** - client sees outgoing requests, server sees incoming requests.

### Bidirectional Tunneling

Both peers can simultaneously use each other as exit nodes:

```
┌──────────────┐              ┌──────────────┐
│   Peer A     │◄───────────► │   Peer B     │
│  NYC, USA    │  Iroh Tunnel │  London, UK  │
└──────────────┘              └──────────────┘
       │                             │
       ↓                             ↓
  Browse from                   Browse from
  London IP                     NYC IP
```

**Use Cases:**
- Geographic load balancing
- Testing from multiple locations
- Redundant exit paths

### Persistent Node Identity

Node identity persists across restarts using `.tunnel_key` and `.tunnel_peer` files:

**`.tunnel_key`** - Your node's secret key (keep private!)
- Generated automatically on first run
- Ensures same Node ID across restarts
- Delete to generate new identity

**`.tunnel_peer`** - Remote peer's public key
- Saved when connection establishes
- Enables server-side reconnection
- Auto-updated when peer changes

**Security Note:** Keep `.tunnel_key` private. It's equivalent to your node's private key.

### Loop Prevention

Automatic detection of routing loops:

```
Peer A → Peer B → Peer A  ❌ BLOCKED
```

**How it works:**
- Detects SOCKS proxy addresses in tunnel requests
- Rejects connections that would create loops
- Logs warnings for visibility

**Example:**
```
⚠️  Loop detected! Rejecting connection to localhost:1080
```

---

## Network Architecture

### Connection Types

Iroh establishes connections using one of three methods:

<details>
<summary><b>1. Direct (Holepunched)</b> - Best performance</summary>

```
┌────────┐                  ┌────────┐
│ Peer A │ ←──UDP Direct──→ │ Peer B │
└────────┘                  └────────┘
```

- **Latency:** Lowest (direct path)
- **Bandwidth:** Full available bandwidth
- **Requirements:** One peer must be publicly reachable OR hole-punching succeeds
- **Common:** Home networks, VPS, same LAN

</details>

<details>
<summary><b>2. Relay</b> - Fallback option</summary>

```
┌────────┐      ┌───────┐      ┌────────┐
│ Peer A │ ←───→│ Relay │ ←───→│ Peer B │
└────────┘      └───────┘      └────────┘
```

- **Latency:** Higher (two hops)
- **Bandwidth:** Limited by relay capacity
- **Requirements:** None (always works)
- **Common:** Symmetric NATs, restrictive firewalls

</details>

<details>
<summary><b>3. Mixed</b> - Asymmetric routes</summary>

```
┌────────┐                  ┌────────┐
│ Peer A │ ──Direct──→      │ Peer B │
│        │      ←──Relay──  │        │
└────────┘                  └────────┘
```

- **Latency:** Variable by direction
- **Bandwidth:** Asymmetric
- **Requirements:** Partial reachability
- **Common:** One peer behind firewall

</details>

### Protocol Messages

The tunnel uses a custom protocol over Iroh streams:

```rust
enum TunnelMessage {
    // Client → Server: Request connection
    Connect { 
        host: String,    // "example.com"
        port: u16        // 443
    },
    
    // Server → Client: Connection established
    Connected,
    
    // Server → Client: Connection failed
    Error { 
        message: String  // "Connection refused"
    },
    
    // Bidirectional: Transfer data
    Data { 
        data: Vec<u8>    // Raw bytes
    },
    
    // Either → Other: Close stream
    Close,
}
```

**Message Flow:**
```
Client                    Server
  │                        │
  │──Connect{host,port}──→ │
  │                        │ [Connects to host:port]
  │                        │
  │←────Connected─────────  │
  │                        │
  │──Data{...}───────────→ │──→ Remote Host
  │                        │
  │←────Data{...}──────────│ ←─ Remote Host
  │                        │
  │──Close───────────────→ │
  │                        │
```

### Health Monitoring

Background monitor ensures connection reliability:

```
┌──────────────────────────┐
│  Health Monitor Thread   │
│  (every 5 seconds)       │
└───────────┬──────────────┘
            │
            ↓
    ┌───────────────┐
    │ Check Status  │
    │ conn.closed? │
    └───────┬───────┘
            │
      ┌─────┴─────┐
      │           │
    Yes          No
      │           │
      ↓           ↓
   Trigger     Continue
Reconnection   Monitoring
```

---

## Configuration Examples

### Personal VPN

Route all traffic through a home server:

**Home Server (has public IP):**
```bash
# Run on home server (203.0.113.10)
tunnel

# Share ticket: 5j7k8m9n...
```

**Laptop (anywhere):**
```bash
# Connect from anywhere
tunnel --peer "5j7k8m9n..."

# Configure system-wide proxy
export ALL_PROXY=socks5://localhost:1080
```

### Geo-Distributed Testing

Test website from multiple locations:

**London Server:**
```bash
tunnel --port 1080
# Share ticket-london
```

**Tokyo Server:**
```bash
tunnel --port 1081
# Share ticket-tokyo
```

**Your Machine:**
```bash
# Terminal 1: London exit
tunnel --peer "ticket-london" --port 2080

# Terminal 2: Tokyo exit
tunnel --peer "ticket-tokyo" --port 2081

# Test from London
curl --socks5 localhost:2080 https://example.com

# Test from Tokyo
curl --socks5 localhost:2081 https://example.com
```

### Secure LAN Access

Access home network services remotely:

**Home Network (192.168.1.0/24):**
```bash
# Run tunnel on home server
tunnel
```

**Remote Location:**
```bash
# Connect to home
tunnel --peer "..."

# Access home services via SOCKS
curl --socks5 localhost:1080 http://192.168.1.100:8080
```

---

## Troubleshooting

<details>
<summary><b>Connection Issues</b></summary>

**Symptom:** `❌ No peer connection available`

**Solutions:**
1. Verify both peers are running
2. Check firewall allows UDP (Iroh uses UDP for QUIC)
3. Wait 10-15 seconds for NAT traversal
4. Try explicit `--peer` ticket instead of discovery
5. Check network connectivity: `ping <peer-ip>`

**Debug:**
```bash
# Enable verbose logging
RUST_LOG=debug tunnel --peer "..."
```

</details>

<details>
<summary><b>Reconnection Failures</b></summary>

**Symptom:** `🔄 Connection lost, attempting reconnection...` never succeeds

**Solutions:**
1. Check `.tunnel_peer` file exists (peer ID persistence)
2. Verify remote peer is running and reachable
3. Check for changed IP addresses (dynamic IPs)
4. Restart both peers to re-establish connection
5. Delete `.tunnel_peer` and reconnect with fresh ticket

**Expected Behavior:**
- Reconnection should succeed within 60 seconds if peer is online
- Exponential backoff maxes out at 60-second intervals

</details>

<details>
<summary><b>Performance Issues</b></summary>

**Symptom:** Slow browsing, high latency

**Diagnostics:**
1. Check connection type in logs:
   - `direct (holepunched)` = Good
   - `relay` = Expect higher latency
2. Test baseline latency: `ping <peer-ip>`
3. Monitor traffic stats in console output

**Optimization:**
- Use peer with good internet connectivity
- Prefer direct connections over relay
- Consider VPS deployment for static IPs
- Close unnecessary SOCKS connections

</details>

<details>
<summary><b>DNS Resolution Failures</b></summary>

**Symptom:** Websites timeout or fail to load

**Solutions:**
1. **Firefox:** Enable "Proxy DNS when using SOCKS v5"
2. **Chrome:** Use `--proxy-pac-url` instead of `--proxy-server`
3. Verify remote peer can resolve DNS: `nslookup google.com`

</details>

<details>
<summary><b>IPv6 Connection Failures</b></summary>

**Symptom:** `❌ Failed to connect to [2404:6800:4007:083d::200e]:80: Network is unreachable (os error 101)`

**Explanation:** The **exit peer** (where traffic exits to the internet) lacks IPv6 connectivity.

**How It Works:**
- SOCKS5 proxy receives IPv6 destination from client
- Request is tunneled to remote peer
- Remote peer attempts to connect to IPv6 address
- If remote peer has no IPv6 support, connection fails

**Solutions:**
1. **Use IPv6-capable exit peer** - Ensure remote peer has IPv6 connectivity
2. **Test peer IPv6 support:**
   ```bash
   # On exit peer, test IPv6 connectivity
   ping6 google.com
   curl -6 https://ipv6.google.com
   ```
3. **Verify dual-stack:**
   - Most modern servers support both IPv4 and IPv6
   - VPS providers often offer dual-stack by default
   - Home networks may require IPv6 enabling via router

**Note:** The tunnel itself fully supports IPv6. The limitation is the exit peer's network connectivity, not the tunnel implementation.

</details>

<details>
<summary><b>Loop Detection Warnings</b></summary>

**Symptom:** `⚠️  Loop detected! Rejecting connection to localhost:1080`

**Explanation:** This is **normal behavior**. The tunnel is preventing an infinite routing loop.

**Common Cause:**
- Browser tries to connect to `localhost:1080` through the proxy itself
- Auto-proxy configuration scripts
- System-wide proxy settings affecting the tunnel itself

**Solution:** This warning can be safely ignored. It protects your system from routing loops.

</details>

---

## Security & Privacy

### Encryption

All traffic is encrypted end-to-end using Iroh's security model:

```
Application Layer:    [HTTP/HTTPS]
    ↓
SOCKS5 Layer:        [SOCKS5 Protocol]
    ↓
Tunnel Protocol:     [Custom Messages]
    ↓
Iroh Layer:          [📦 QUIC + Noise Encryption]
    ↓
Transport Layer:     [UDP]
```

**Cryptography:**
- **QUIC:** TLS 1.3 encryption for transport
- **Noise Protocol:** Authenticated encryption framework
- **Ed25519:** Node identity and authentication
- **ChaCha20-Poly1305:** Symmetric encryption

### Privacy Considerations

⚠️ **What the remote peer can see:**
- Destination IP addresses and ports
- Unencrypted traffic (plain HTTP)
- Traffic timing and volume
- Your node's public key (anonymized)

✅ **What the remote peer CANNOT see:**
- HTTPS content (encrypted end-to-end to destination)
- Your physical IP address (if using NAT/relay)
- Local network details

### Security Best Practices

1. **Trust Your Peer:** Only connect to peers you trust
2. **Use HTTPS:** Always prefer HTTPS over HTTP for sensitive data
3. **Protect .tunnel_key:** Treat as private key - don't share or commit to git
4. **Monitor Logs:** Watch for unusual connection patterns
5. **Firewall Rules:** Restrict tunnel to localhost only (default)

### Threat Model

**Protected Against:**
- Network eavesdropping (encrypted)
- MITM attacks (authenticated)
- IP address exposure (traffic exits from peer)
- Connection fingerprinting (QUIC randomization)

**NOT Protected Against:**
- Malicious remote peer (can see/modify HTTP traffic)
- Endpoint compromise (if peer is compromised)
- Traffic analysis (timing/volume patterns)

---

## Development

### Building

```bash
# Development build
cargo build --bin tunnel

# Release build (optimized)
cargo build --release --bin tunnel

# Run tests
cargo test

# Check for errors
cargo check --bin tunnel
```

### Code Structure

```
iroh-socks5-proxy/
├── src/
│   ├── connection/
│   │   ├── logger.rs           # Connection logging utilities
│   │   └── mod.rs
│   ├── http/
│   │   ├── parser.rs           # HTTP request parser
│   │   └── mod.rs
│   ├── socks5/
│   │   ├── protocol.rs         # SOCKS5 protocol implementation
│   │   └── mod.rs
│   ├── tls/
│   │   ├── sni.rs              # TLS SNI extraction
│   │   └── mod.rs
│   ├── tunnel/
│   │   ├── connection.rs       # Connection management & monitoring
│   │   ├── persistence.rs      # Key & peer ID persistence
│   │   ├── protocol.rs         # Custom tunnel protocol messages
│   │   ├── relay.rs            # Bidirectional data relay
│   │   ├── socks.rs            # SOCKS5 client handling
│   │   ├── state.rs            # Tunnel state management
│   │   └── mod.rs
│   ├── utils/
│   │   ├── logging.rs          # Logging helpers
│   │   └── mod.rs
│   ├── lib.rs                  # Library exports
│   └── main.rs                 # Tunnel binary entry point
├── Cargo.toml
├── .gitignore
└── README.md
```

### Contributing

Contributions welcome! Areas of interest:

- SOCKS5 UDP ASSOCIATE command
- Connection statistics / monitoring dashboard
- Multi-hop routing
- Bandwidth limiting
- GUI application
- Performance optimizations

### Running Integration Tests

```bash
# Terminal 1: Start server
cargo run --bin tunnel

# Terminal 2: Start client
cargo run --bin tunnel -- --peer "<ticket>"

# Terminal 3: Test
curl --socks5 localhost:1080 https://ifconfig.me
```

---

## Comparison with Alternatives

| Feature | Iroh SOCKS5 | SSH Tunnel | WireGuard | Tailscale |
|---------|-------------|------------|-----------|-----------|
| NAT Traversal | ✅ Built-in | ❌ Manual | ❌ Manual | ✅ Built-in |
| Zero Config | ✅ Yes | ❌ Requires setup | ❌ Requires setup | ✅ Yes |
| Encryption | ✅ QUIC+Noise | ✅ SSH | ✅ WireGuard | ✅ WireGuard |
| Auto-Reconnect | ✅ Yes | ❌ No | ⚠️  Limited | ✅ Yes |
| Root Required | ✅ No | ✅ No | ❌ Yes | ❌ Yes (usually) |
| P2P Direct | ✅ Yes | ❌ Client-Server | ✅ Yes | ✅ Yes |
| Speed | ⚠️ Good | ⚠️ Good | ✅ Excellent | ✅ Excellent |
| Complexity | ✅ Simple | ⚠️ Moderate | ❌ Complex | ✅ Simple |

---

## Frequently Asked Questions

<details>
<summary><b>Is this a VPN?</b></summary>

Not quite. It's a **SOCKS5 proxy** which:
- Routes application traffic (not all traffic)
- Works without root/admin privileges
- Doesn't create virtual network interfaces
- Requires application configuration

A traditional VPN routes ALL traffic and operates at the IP layer.

</details>

<details>
<summary><b>Can I use this for torrenting?</b></summary>

**Technical answer:** SOCKS5 supports TCP, which many torrent clients can use.

**Practical answer:** Not recommended. The tunnel adds overhead and latency that degrades torrent performance. Use a dedicated VPN or seedbox instead.

</details>

<details>
<summary><b>What's the performance impact?</b></summary>

**Latency:** Adds 10-100ms depending on peer distance and connection type
**Bandwidth:** Minimal overhead (~5%) for data transfer
**CPU:** Low (QUIC encryption is efficient)

For typical web browsing: imperceptible
For low-latency gaming: noticeable
For bulk downloads: slight slowdown

</details>

<details>
<summary><b>Does it work on mobile?</b></summary>

**iOS/Android:** Not directly. Requires:
1. Rust cross-compilation for mobile
2. Platform-specific SOCKS integration
3. Background service permissions

Current implementation targets desktop/server platforms.

</details>

<details>
<summary><b>Can the tunnel operator see my passwords?</b></summary>

**HTTPS sites (https://):** ❌ No, encrypted end-to-end
**HTTP sites (http://):** ✅ Yes, transmitted in plain text

**Recommendation:** Only use with trusted peers, prefer HTTPS sites.

</details>

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Acknowledgments

Built with:
- [Iroh](https://github.com/n0-computer/iroh) - Peer-to-peer networking library
- [Tokio](https://tokio.rs) - Asynchronous runtime
- [Clap](https://github.com/clap-rs/clap) - Command line parsing

---

## Support

- **Issues:** [GitHub Issues](https://github.com/noelaugustin/iroh-socks5-proxy/issues)
- **Discussions:** [GitHub Discussions](https://github.com/noelaugustin/iroh-socks5-proxy/discussions)
- **Email:** [Your contact email]

---

**Made with ❤️ using Rust and Iroh**
