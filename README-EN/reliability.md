# Reliability (UDP)

UDP is connectionless and does not guarantee delivery. This project adds application-level reliability mechanisms:

## 1) Acknowledgements (ACK)

- Set `requires_ack = true` to request confirmation.
- On successful parse/acceptance, the receiver sends an `Ack`:

```json
{
  "message_type": "Ack",
  "ack_for": 42,
  "timestamp": 1234567892,
  "sender_addr": "127.0.0.1:8080"
}
```

- `ack_for` references the confirmed `sequence_number`.

## 2) Sequence Numbers & Deduplication

- Sender assigns increasing `sequence_number` to reliability-sensitive messages.
- Receiver maintains a recent cache window and drops duplicates.

## 3) Retransmission

- If no `Ack` within timeout, send `Retransmit` or resend the original message.
- Suggested strategy (tunable):
  - Initial timeout: `500ms`
  - Max retries: `3`
  - Backoff: exponential (`500ms`, `1s`, `2s`)

Example request:

```json
{
  "message_type": "Retransmit",
  "payload": { "sequence_number": 42 },
  "timestamp": 1234567893
}
```

## 4) Out-of-Order & Loss

- UDP packets can arrive out of order. Drive logic by `sequence_number` and type rather than ordering assumptions.
- For handshake/auth and critical data, use `requires_ack` and retransmit.

## 5) NAT & Port Changes

- Peers are identified by `SocketAddr`. If a client restarts or NAT changes the source port, treat it as a new peer and re-handshake.

## 6) Security & Rate Limiting

- Rate limit `requires_ack` messages to avoid ACK storms.
- Validate `Retransmit` sources; accept retransmit requests only from known peers.

## 7) Logging & Observability

- At `debug` level, the system logs send/receive and handling details, useful for observing ACK/retransmit behavior.
- Lower log level in production to reduce overhead.