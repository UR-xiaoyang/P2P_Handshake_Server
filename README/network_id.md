# 网络ID（network_id）握手与隔离说明

本文档说明项目中关于网络ID的最新约定：网络ID不再由服务端配置与固定，而是由握手发起方（客户端）在 `HandshakeRequest` 中主动提供并驱动网络隔离与确认。

## 设计原则

- 客户端驱动：`network_id` 必须由发起握手的一方在其 `NodeInfo.metadata` 中提供。
- 明确校验：服务端在处理握手请求时，如果缺少或为空的 `network_id`，将直接拒绝握手并返回错误。
- 响应回显：服务端在 `HandshakeResponse` 中回显客户端提供的 `network_id`，便于客户端确认加入的是正确的网络。
- 可选二次校验：当本端节点（作为主动发起方）也设置了本地 `network_id` 时，收到对端 `HandshakeResponse` 后会比对；若不匹配则拒绝连接。

## 握手流程（简化）

1. 客户端发送 `HandshakeRequest`，在 `NodeInfo.metadata["network_id"]` 写入目标网络ID。
2. 服务端验证：
   - 缺少或为空：返回 `Error`，拒绝握手；
   - 存在：接受并认证，通过后发送 `HandshakeResponse`，其中 `node_info.metadata["network_id"]` 回显客户端的网络ID。
3. 客户端收到响应后比对网络ID；不一致则主动断开。

## 配置影响

- 服务端不再依赖配置文件中的 `network_id` 进行固定设置；接受请求时以客户端提供的 `network_id` 为准。
- 如果你的节点需要主动连接其他节点（充当发起方），也应在本地节点的 `NodeInfo.metadata` 中设置 `network_id`，否则可能被对端拒绝。

> 注：当前代码中 `Config` 里仍保留 `network_id` 字段，但服务端默认不会将其注入到 `local_node_info.metadata`。它主要用于“本端主动发起连接并校验响应”这类场景，可根据需要自行写入。

## 示例：客户端设置与校验

```rust
use p2p_handshake_server::protocol::{NodeInfo, Message, MessageType, HandshakeResponse};

// 构建客户端 NodeInfo，并设置 network_id
let mut client_info = NodeInfo::new("test_client".to_string(), local_addr);
client_info.add_capability("test".to_string());
client_info.add_metadata("network_id".to_string(), "p2p_default".to_string());

// 发送握手请求
let req = Message::new_with_ack(
    MessageType::HandshakeRequest,
    serde_json::to_value(&client_info)?,
    local_addr,
    1,
);

// 收到响应后校验回显的 network_id
if let Ok(resp) = serde_json::from_value::<HandshakeResponse>(response.payload.clone()) {
    let remote_net = resp.node_info.metadata.get("network_id");
    let local_net = client_info.metadata.get("network_id");
    if remote_net == local_net {
        println!("网络ID匹配: {:?}", remote_net);
    } else {
        println!("网络ID不匹配: 本地={:?}, 对端={:?}", local_net, remote_net);
        // 断开或重试
    }
}
```

## 服务端行为摘要

- 处理 `HandshakeRequest` 时：
  - 若 `metadata["network_id"]` 缺失或为空，返回错误并拒绝认证；
  - 认证成功后在 `HandshakeResponse` 中回显该 `network_id`。
- 处理 `HandshakeResponse` 时（当本端作为发起方）：
  - 若本地已设置 `network_id`，将与对端回显的值比对，不一致则拒绝连接。

## 进阶建议

- 强认证：可在 `metadata` 同时携带 `join_token` 或签名，服务端在握手时一并校验以增强跨网络隔离。
- 观测与日志：为便于排查，建议在日志中打印握手双方的 `network_id`，并对不匹配场景进行清晰告警。

### 强认证方案（可选）

为避免不同网络的客户端误接入或恶意接入，推荐在 `metadata` 携带可信凭据并在握手过程中进行校验。

1) HMAC `join_token`（共享密钥）

- 字段：
  - `metadata["join_token"]`：HMAC 计算得到的令牌。
  - `metadata["join_ts"]`：令牌生成的时间戳（秒）。
- 建议计算方式：`join_token = HMAC_SHA256(secret, node_id | network_id | join_ts)`。
- 服务端校验：
  - 若已配置 `join_secret`（共享密钥），则：
    - 检查 `join_ts` 与当前时间差（例如 ≤ 120 秒）以防重放；
    - 使用同样的拼接规则计算期望 HMAC，与客户端提供的 `join_token` 比对；
    - 可维护一个滑动窗口缓存 `(node_id, join_ts)` 或 nonce，避免重复请求。
- 伪代码示例：

```rust
// 伪代码，仅示意
fn make_join_token(secret: &[u8], node_id: &Uuid, network_id: &str, ts: i64) -> String {
    let msg = format!("{}|{}|{}", node_id, network_id, ts);
    let mac = hmac_sha256(secret, msg.as_bytes());
    hex_encode(mac)
}

fn verify_join_token(secret: &[u8], node_id: &Uuid, network_id: &str, ts: i64, token_hex: &str) -> bool {
    let expected = make_join_token(secret, node_id, network_id, ts);
    constant_time_eq(token_hex.as_bytes(), expected.as_bytes())
}
```

2) 数字签名 `join_sig`（公钥验证）

- 字段：
  - `metadata["join_sig"]`：对 `(node_id | network_id | join_ts)` 的签名（如 Ed25519）。
  - `metadata["join_ts"]`：令牌时间戳（秒）。
  - `metadata["pubkey_id"]`：可选，用于标识所用公钥；服务端持有可信公钥列表。
- 服务端校验：
  - 根据 `pubkey_id` 选取公钥（或使用统一信任锚）；
  - 验证签名合法性，并检查 `join_ts` 时间窗口与重放缓存。

3) 字段命名建议与兼容性

- 建议统一：`network_id`、`join_token` 或 `join_sig`、`join_ts`、`pubkey_id`。
- 为兼容不同客户端，服务端可按优先级依次尝试：签名校验 > HMAC 校验 > 仅基于 `network_id` 的弱校验。

### 配置建议（可选）

尽管服务端不固定 `network_id`，但为启用强认证，服务端可在本地配置：

- `join_secret`：共享密钥，用于 HMAC 校验；
- `trusted_pubkeys`：可信公钥列表，用于签名校验；

这两个配置是可选的；若未设置，服务端将仅进行 `network_id` 的存在性检查与回显，不做强认证。

### 迁移指引

- 现有部署可直接按“客户端驱动的 network_id”工作；
- 若需要逐步引入强认证：
  - 客户端先开始填充 `join_ts` 与 `join_token`/`join_sig`；
  - 服务端启用对应的校验配置后，观察日志与接入效果；
  - 最终将“不携带凭据”的请求回退为拒绝（灰度生效）。