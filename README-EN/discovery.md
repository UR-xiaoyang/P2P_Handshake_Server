# Peer Discovery

This mechanism discovers peers in a local network or within a configured port range to build richer P2P topologies.

## Configuration

`config.json` example:

```json
{
  "enable_discovery": true,
  "discovery_port_range": [8081, 8090]
}
```

- `enable_discovery`: toggle discovery.
- `discovery_port_range`: probe/broadcast within this range.

## Approaches

1. Active probing:
   - Send `DiscoveryRequest` to candidate ports in `discovery_port_range`.
   - On `DiscoveryResponse`, record node info and proceed to handshake.

2. Broadcast/multicast (optional):
   - Broadcast or multicast `DiscoveryRequest` in the local subnet (environment support required).
   - Listen on multicast addresses for peer responses.

3. Responses & rate limiting:
   - Respond with `DiscoveryResponse` including `NodeInfo`.
   - Rate limit to avoid storms.

## Relation to Handshake

- Discovery finds candidates; handshake authenticates and enables communication.
- Recommended to send `HandshakeRequest` immediately after a successful discovery (with `requires_ack`).

## State & Cleanup

- Periodically purge unreachable or unresponsive candidates to reduce resource usage.

## Current Status

- Configuration fields are in place. Broadcast/multicast and port scanning strategies can be implemented progressively based on deployment needs.