# Client Mechanics

This document describes the typical UDP client workflow and key implementation points. See `examples/simple_client.rs`.

## Workflow

1. Create and bind a `UdpSocket` (OS selects an ephemeral local port).
2. Prepare server address (e.g., `127.0.0.1:8080`).
3. Send `HandshakeRequest` (prefer `requires_ack = true`):
   - Wait for `Ack(ack_for=seq)` and `HandshakeResponse`.
4. On success, send `Data` (request ACK if important).
5. Send `Ping`; receive `Pong` to assess latency and connectivity.
6. Send `Disconnect` to gracefully close.

## Key Points

- `sequence_number`: assign for reliability-sensitive messages to enable ACK and dedup.
- `requires_ack`: recommended for handshake, control messages, and important payloads.
- Receive loop: after sending, wait and match by type and `ack_for`.
- Timeouts & retries: on missing critical replies, retry or send `Retransmit`.

## Example Payload

```json
{
  "message": "Hello from UDP client!",
  "timestamp": 1760000000
}
```

## Logging & Troubleshooting

- Server-side `RUST_LOG=debug` helps observe interactions.
- Common errors:
  - `WSAECONNRESET (10054)`: In UDP contexts often means peer unreachable or port changed; retry or re-handshake.
  - `Failed to receive UDP packet`: typically after client disconnect; reduce log level or ignore.