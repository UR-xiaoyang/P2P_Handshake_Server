# Routing (Integrated)

`router.rs` is integrated with the server lifecycle and supports multi-hop routing and controlled broadcast for `Data` messages. This document describes the routing module’s design, integration points, message format, tests, and usage.

## Overview

- Uses a `RoutingTable` for next-hop selection with broadcast as a fallback.
- Handshake and discovery responses update the routing table; disconnect removes related routes.
- Built-in message deduplication cache with periodic cleanup to prevent loops and repeated forwarding.
- Provides a server API `P2PServer::send_routed_data` for targeted routed delivery.

## Key Components

- `RoutingTable`
  - Maintains mappings `destination_node_id -> next_hop_node_id` with `distance`.
  - Updates only when a new path is shorter; supports removing routes for a destination or all routes via a given next hop.
- `MessageRouter`
  - `route_message`: deliver locally when destined for self; otherwise wrap and forward.
  - `forward_message`: performs dedup, TTL (`max_hops`), next-hop selection, or broadcast fallback.
  - `broadcast_message`: broadcasts to all authenticated peers (excluding the source).
  - `handle_local_message`: logic for when destination is local (currently logs).
  - `update_routing_table` / `remove_node_routes` / `get_routing_table_snapshot`.
  - Message cache: `is_message_cached`, `cache_message_id`, and `start_cache_cleanup_task`.
- `PeerManager`
  - Manages peer addition, authentication state, and connections; routing broadcast targets the “authenticated” set.
- `P2PServer`
  - On successful handshake, registers a direct route (`distance = 1`).
  - On `DiscoveryResponse`, registers second-level routes announced by neighbors (`distance = 2`).
  - On `Data`, attempts routing; cleans up routes on disconnect.
  - Exposes `send_routed_data` for targeted sends.

## Routing Message Format

Routing is performed by wrapping `RoutedMessage` inside `MessageType::Data`:

```json
{
  "message_type": "Data",
  "payload": {
    "original_message": { /* original business payload */ },
    "source_node": "uuid",
    "destination_node": "uuid",
    "hop_count": 1,
    "max_hops": 10,
    "route_id": "uuid"
  }
}
```

- `hop_count` increments on each forward and terminates when exceeding `max_hops`.
- `route_id` is used by the dedup cache to prevent repeated processing and loop expansion.

## Server Integration Workflow

- Handshake success (`HandshakeResponse`)
  - Register a direct route: `distance = 1`, next hop is the peer.
- Node discovery (`DiscoveryResponse`)
  - Register second-level routes announced by the neighbor: `distance = 2`, next hop is that neighbor.
- Data messages (`Data`)
  - Destination is local: handle locally.
  - No next hop: broadcast to all authenticated peers (skip the source).
  - Unreachable next hop: remove the route and then broadcast.
- Disconnect (`Disconnect` or unexpected disconnection)
  - Remove routes whose destination is the node and all routes that traverse the node.

## Usage Example (Server-side targeted send)

`P2PServer` provides:

```rust
pub async fn send_routed_data(
    &self,
    destination_node: uuid::Uuid,
    payload: serde_json::Value,
    max_hops: u32,
) -> anyhow::Result<()>
```

Example call:

```rust
server.send_routed_data(
    target_node_id,
    serde_json::json!({"msg":"hello"}),
    8,
).await?;
```

## Tests & Verification

Async end-to-end tests are included in `src/router.rs`:

- `test_forward_via_next_hop`
  - Setup local and next-hop UDP sockets, configure a route, and verify the peer receives the wrapped `Data`.
- `test_broadcast_when_no_route`
  - In the absence of a route, verify both peers receive broadcasted data messages.
- `test_unreachable_next_hop_removes_route_and_broadcasts`
  - Configure an unreachable next hop, assert the route is removed and broadcast is used.

Run:

```bash
cargo test
```

Set `RUST_LOG=debug` to observe routing behavior and broadcast results.

## Notes & Next Steps

- Local data handling (`handle_local_message`) currently logs; extend with business logic as needed.
- Consider richer route discovery and distance metrics (RTT/loss/scoring) to dynamically update optimal paths.
- Add examples demonstrating multi-node connectivity and routing visualization.