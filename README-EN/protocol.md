# Protocol (UDP)

This document details message types, format, field semantics, and handshake flow with reliability.

## Message Types (`MessageType`)

- `HandshakeRequest`: Client-initiated handshake including node info.
- `HandshakeResponse`: Server response with authentication/acceptance details.
- `Ping` / `Pong`: Health check and RTT measurement.
- `DiscoveryRequest` / `DiscoveryResponse`: Optional peer discovery.
- `Data`: Generic payload message.
- `Disconnect`: Graceful disconnect notification.
- `Error`: Error reporting with code and message.
- `Ack`: Acknowledgement for reliability.
- `Retransmit`: Request for retransmission when packet loss occurs.

## Message Structure (`Message`)

Messages use JSON. Recommended fields matching the implementation:

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
- `id`: Unique message ID (UUID).
- `message_type`: One of the types above.
- `timestamp`: Seconds since epoch.
- `payload`: Free-form JSON body.
- `sender_addr`: String form of `SocketAddr` for the sender.
- `sequence_number`: Monotonic number for deduplication and ACK matching.
- `requires_ack`: Ask peer to acknowledge the message.
- `ack_for`: When this is an `Ack`, points to the confirmed `sequence_number`.

## Handshake Flow (with ACK)

```
Client                            Server
  | -- HandshakeRequest (requires_ack=true, seq=1) -->
  | <-- Ack (ack_for=1) -----------------------------
  | <-- HandshakeResponse (seq=2, requires_ack=true) -
  | -- Ack (ack_for=2) ----------------------------->
  |                 [Authenticated]
```

## Heartbeat & Data

- `Ping`/`Pong`: Either side can initiate; measure health and latency.
- `Data`: Carry application payload. Use `requires_ack` when delivery matters.

## Errors & Disconnect

- `Error`: Parse errors, permission issues, invalid messages.
- `Disconnect`: Mark peer as disconnected and clean up server-side state.

## Sequence Numbers & Idempotency

- Assign increasing `sequence_number` for reliable messages.
- Receiver keeps a recent window and drops duplicates to ensure idempotency.