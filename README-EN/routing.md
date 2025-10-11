# Routing (Planned)

`router.rs` contains interfaces such as `route_message`, `forward_message`, and `broadcast_message` for multi-hop forwarding and routing table maintenance in larger P2P networks. This document outlines goals and draft design.

## Goals

- Forward messages across multi-hop networks.
- Maintain dynamic routing tables selecting optimal next hops (latency/reliability/bandwidth).
- Provide message deduplication cache to reduce loops and duplication.

## Core Capabilities (Draft)

- `route_message(message)`: choose next hop or local handling.
- `forward_message(routed_message)`: send to the next hop.
- `broadcast_message(routed_message)`: controlled broadcast with dedup and TTL.
- `handle_local_message(message)`: when destination is local.
- `update_routing_table(node_id, next_hop)`: maintain routes.
- `remove_node_routes(node_id)`: cleanup routes.
- `get_routing_table_snapshot()`: observe current route set.
- Cache utilities: `is_message_cached`, `cache_message_id`, and a periodic cleanup task.

## Integration Plan

- After handshake, register reachable neighbors and link quality with the routing layer.
- When data is not locally deliverable, delegate to routing for the next hop.
- Use ACK/retransmit stats to adjust path choices dynamically.

## Status & Notes

- Not yet integrated with the main server flow; functions may emit "unused" warnings.
- Future integration should be performance-aware and cautious with broadcast/multi-hop logic to avoid storms.