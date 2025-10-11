# Overview

This project is a P2P handshake server written in Rust. It has been migrated from TCP to UDP to reduce latency and better fit peer-to-peer networking scenarios. The system is organized in modular components:

- `network.rs`: UDP networking using `UdpSocket` for `send_to`/`recv_from`; maintains a map of known peer addresses.
- `protocol.rs`: Protocol definitions with message types and structure, including reliability fields (`Ack`, `Retransmit`, `sequence_number`, `requires_ack`, `ack_for`).
- `peer.rs`: Peer management indexed by `SocketAddr` to support connectionless operation; lookup and lifecycle.
- `server.rs`: Main loop that receives packets, parses messages, dispatches handlers, and auto-sends ACK when required.
- `examples/simple_client.rs`: UDP client example demonstrating handshake, data, ping/pong, and disconnect.
- `router.rs`: Planned routing utilities for multi-hop forwarding and routing table maintenance.

## Motivation: Why UDP

- Lower latency: No connection setup/keep-alive overhead; direct packet delivery.
- P2P-friendly: Better suited for broadcast/multicast and discovery; reduced connection management cost.
- Controlled reliability: Add ACK/retransmit only where needed at the application layer.

## High-Level Data Flow

1. Client sends a `HandshakeRequest` (optionally with `requires_ack=true`) via `UdpSocket::send_to` to the server address.
2. Server receives with `UdpSocket::recv_from` and parses into a `Message`.
3. Server resolves/creates a `Peer` using the source `SocketAddr`, then dispatches by `MessageType`.
4. If the message requires an ACK, the server replies with `Ack`.
5. Both sides exchange `Data`, `Ping/Pong`, and `Disconnect` messages as needed.

## Key Capabilities

- Connectionless UDP transport driven by address and packet semantics.
- Application-level reliability via `Ack`, `Retransmit`, and `sequence_number`.
- Address-driven peer index using `SocketAddr` to associate packets with peers.
- Background tasks: heartbeat, cleanup, and stats using Tokio timers and `join!`.

## Running & Logging

Enable detailed logs during development:

```powershell
$env:RUST_LOG="debug"; cargo run --bin p2p_server
```

Run the client example:

```powershell
cargo run --example simple_client
```

Logs will show handshake, ACK, data, ping/pong, and disconnect interactions end-to-end.