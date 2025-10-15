# `network_id`: Isolation & Configuration

`network_id` is a key concept in this P2P system. It enforces network isolation, improves security, and keeps networks clean. This document explains its role, configuration, and future direction.

## Purpose: The “pass” for network isolation

Treat `network_id` as a passphrase for a specific P2P network. When handling a `HandshakeRequest`, the server strictly validates that the client’s `network_id` matches the server’s configured `network_id`.

Benefits:

1. Isolation across environments: run multiple independent P2P networks (production/test/dev), each with a distinct `network_id`, to prevent cross-network contamination.
   - Production: `network_id: "prod-mainnet"`
   - Test: `network_id: "test-sidechain"`
2. Security: basic access control — only nodes with the correct `network_id` pass validation and join.

## Configuration

The server’s `network_id` is loaded at startup.

1) Via config file (recommended)

Use `--config`/`-c` to point to a JSON config (e.g., `config.json`). The server reads `network_id` from it.

`config.json` example:

```json
{
  "listen_address": "127.0.0.1:8080",
  "max_connections": 100,
  "network_id": "test-net"
}
```

Start:

```powershell
cargo run -- --config config.json
```

In this mode, the expected `network_id` is `"test-net"`.

2) Default (when no config file)

If no config file is provided, the server uses the default defined in code (`Config::default()`), which is `"p2p_default"`.

Start:

```powershell
cargo run
```

In this mode, the expected `network_id` is `"p2p_default"`.

Important: After changing `config.json`, restart the server to apply changes. Hot reload is not supported.

## Common Issue: “Network ID mismatch”

The log `Network ID mismatch: expected <A>, received <B>` indicates:
- Server loaded `<A>`
- Client sent `<B>`
- Often due to not loading the correct config or mismatch between client and server.

Fix:
- Ensure the server is started with `--config config.json`.
- Verify the same `network_id` in `config.json` and the client.
- Restart the server to apply the latest configuration.

## Future: Multi-network bootstrapping

Currently, one server instance serves a single `network_id`. A future direction is to support bootstrapping multiple networks simultaneously.

Design sketch:
- Remove the global server `network_id`.
- Maintain dynamic peer pools partitioned by `network_id`.
- On handshake/discovery, operate within the pool indicated by the request’s `network_id`.

This would improve flexibility and utilization, moving toward multi-tenant bootstrapping.