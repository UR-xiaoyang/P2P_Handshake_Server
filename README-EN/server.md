# Server Mechanics

This document describes server initialization, UDP main loop, message handling, and background tasks.

## Initialization

- Read configuration (`Config`) with listen address and max connections:
  - `listen_address` (e.g., `127.0.0.1:8080`)
  - `max_connections`
- Bind `UdpSocket`; logs show `UDP manager bound to <addr>`.

## Main Loop (Receive Packets)

1. `recv_from` to get `(buffer, source_addr)`.
2. Parse into `Message` including type, payload, and reliability fields.
3. Resolve/create `Connection` and `Peer` (indexed by `SocketAddr`).
4. Dispatch to `handle_message(message)`.
5. Log warnings/errors and clean up state when needed.

## Message Handling (`handle_message`)

- Common: If `requires_ack = true`, send `Ack`.
- `HandshakeRequest`: Validate and register node info, reply with `HandshakeResponse`.
- `HandshakeResponse`: Update peer to authenticated state.
- `Ping`: Reply with `Pong`.
- `Data`: Process `payload`; optionally send business-level confirmation (or just ACK).
- `DiscoveryRequest/Response`: Discovery path (optional/planned).
- `Disconnect`: Mark peer disconnected; initiate cleanup.
- `Error`: Log/report appropriately.
- `Retransmit`: Look up by sequence number and resend or report error.

## Background Tasks

- Heartbeat: Periodically send health checks; logs include "sending heartbeat to N peers".
- Cleanup: Remove inactive or disconnected peers; logs include "peer cleanup".
- Stats: Periodically emit counts of peers by state.
- Combined with `tokio::join!(heartbeat_task, cleanup_task, stats_task)`:
  - Note: Example code may warn about `unused_must_use`; handle each result in production.

## Errors & Logging

- Typical: `Failed to receive UDP packet` often occurs after client disconnect; downgrade to `debug` or suppress as needed.
- Bind failures: port in use or permission issues; change port or adjust privileges.

## Graceful Shutdown

- Console interruption (e.g., `Ctrl+C`) triggers exit.
- Clean up resources and print exit logs (Windows may show `STATUS_CONTROL_C_EXIT`).