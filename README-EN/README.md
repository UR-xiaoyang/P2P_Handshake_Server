# P2P Handshake Server (UDP Edition)

A high-performance P2P handshake server written in Rust, migrated from TCP to UDP for lower latency and better fit for peer-to-peer networking.

## Features

- High-performance async networking with `Tokio`
- Complete P2P handshake protocol with node authentication
- Peer discovery (config-ready; broadcasting/MDNS optional)
- Intelligent message routing (module planned)
- Efficient peer lifecycle management
- JSON-based flexible configuration
- Comprehensive logging
- Robust error handling
- UDP transport with no connection overhead
- Reliability additions: ACK, Retransmit, sequence numbers
- Address-driven peer management via `SocketAddr`

## Quick Start

### Prerequisites

Install Rust (stable):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Build

```bash
git clone <repository-url>
cd P2P_Handshake_Server
cargo build --release
```

### Run the Server

```powershell
cargo run --bin p2p_server
```

With custom config:

```powershell
cargo run --bin p2p_server -- --config config.json --address 127.0.0.1:8080
```

Enable debug logs:

```powershell
# Method 1: CLI argument (takes precedence)
cargo run --bin p2p_server -- --DEBUG

# Method 2: Environment variable (effective when no CLI log level is set)
$env:RUST_LOG="debug"; cargo run --bin p2p_server
```

### Run the Client Example

```powershell
cargo run --example simple_client
```

## Configuration

`config.json` example:

```json
{
  "listen_address": "127.0.0.1:8080",
  "max_connections": 100,
  "heartbeat_interval": 30,
  "connection_timeout": 60,
  "discovery_port_range": [8081, 8090],
  "enable_discovery": true
}
```

### CLI Options

```bash
p2p_server [OPTIONS]

OPTIONS:
  -a, --address <ADDRESS>         Server listen address [default: 127.0.0.1:8080]
  -m, --max-connections <NUMBER>  Max connections [default: 100]
  -c, --config <FILE>             Config file path
      --TRACE                     Set log level to TRACE (mutually exclusive)
      --DEBUG                     Set log level to DEBUG (mutually exclusive)
      --INFO                      Set log level to INFO (mutually exclusive)
      --WARN                      Set log level to WARN (mutually exclusive)
      --ERROR                     Set log level to ERROR (mutually exclusive)
  -h, --help                      Show help
```

## Protocol

### Message Types

- `HandshakeRequest` / `HandshakeResponse`
- `Ping` / `Pong`
- `DiscoveryRequest` / `DiscoveryResponse`
- `Data`
- `Error`
- `Disconnect`
- `Ack` (reliability)
- `Retransmit` (reliability)

### Message Format (JSON)

```json
{
  "id": "uuid",
  "message_type": "MessageType",
  "timestamp": 1234567890,
  "payload": {},
  "sender_addr": "127.0.0.1:8080",
  "sequence_number": 1,
  "requires_ack": false,
  "ack_for": null
}
```

Field notes:
- `sequence_number`: used for deduplication and ACK matching
- `requires_ack`: request peer to acknowledge reception
- `ack_for`: when `Ack`, points to the confirmed `sequence_number`

### Handshake Flow (with ACK)

```
Client                            Server
  | -- HandshakeRequest (requires_ack=true, seq=1) -->
  | <-- Ack (ack_for=1) -----------------------------
  | <-- HandshakeResponse (seq=2, requires_ack=true) -
  | -- Ack (ack_for=2) ----------------------------->
  |                 [Authenticated]
```

## Architecture

- `network.rs`: UDP networking, `send_to` / `recv_from`, known peers map
- `protocol.rs`: message types and structure with reliability fields
- `peer.rs`: peer management via `SocketAddr` index
- `server.rs`: main loop receiving packets, parsing, handling, auto-ACK
- `examples/simple_client.rs`: UDP client with handshake, data, ping/pong, disconnect
- `router.rs`: planned routing utilities (not yet integrated)

## Usage as a Library

```toml
[dependencies]
p2p_handshake_server = { path = "path/to/P2P_Handshake_Server" }
tokio = { version = "1.0", features = ["full"] }
```

```rust
use p2p_handshake_server::{Config, P2PServer};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::new(
        "127.0.0.1:8080".parse::<SocketAddr>()?,
        50,
    );
    let mut server = P2PServer::new(config).await?;
    server.run().await?;
    Ok(())
}
```

Connect to another peer:

```rust
server.connect_to_peer("127.0.0.1:8081".parse()?).await?;
```

Get server stats:

```rust
let stats = server.get_stats().await;
println!("Peers: {}", stats.peer_stats.total_peers);
```

## Reliability (UDP)

- Use `requires_ack` and `Ack` for important messages
- Maintain `sequence_number` for idempotency and deduplication
- Retransmit on timeout (e.g., 500ms, 1s, 2s; max 3 tries)
- Handle out-of-order packets logically by type and sequence
- NAT/port changes may require re-handshake due to new `SocketAddr`

## Performance

- Async I/O with Tokio
- Efficient serialization and transmission
- Connection lifecycle management (address-driven)
- Concurrency-friendly design

## Security

- Message size limits (default 1MB)
- Connection limits
- Timeouts and cleanup
- Error recovery

## Troubleshooting

1. Port in use
   ```
   Error: failed to bind 127.0.0.1:8080
   ```
   Solution: change port or stop the process using it

2. Timeout
   ```
   Error: connection timeout to x.x.x.x:xxxx
   ```
   Solution: check network and address correctness

3. Handshake failure
   ```
   Error: handshake failed: node ID already exists
   ```
   Solution: ensure unique node IDs

4. UDP receive errors after client disconnect
   ```
   ERROR p2p_server::server] Failed to receive UDP packet
   ```
   Note: common when server keeps receiving after client closes; reduce log level or downgrade to debug for this path.

## Migration Guide (TCP → UDP)

- Replace `TcpStream`/`TcpListener` with `UdpSocket` and `send_to`/`recv_from`
- Add reliability via `Ack`/`Retransmit`, `requires_ack`, `sequence_number`
- Manage peers by `SocketAddr`; new source port implies a new peer
- Initiate connections by sending `HandshakeRequest` (no stream establish)
- Client example demonstrates handshake → data → ping/pong → disconnect