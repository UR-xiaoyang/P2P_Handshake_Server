# 协议规范（UDP版）

本文档详细阐述消息类型、消息格式、字段含义与握手流程，覆盖 UDP 场景下的可靠性增强机制。

## 消息类型（`MessageType`）

- `HandshakeRequest`：握手请求，客户端发起，包含节点信息。
- `HandshakeResponse`：握手响应，服务器返回，包含认证结果与必要信息。
- `Ping` / `Pong`：心跳消息，用于连通性与延迟检测。
- `DiscoveryRequest` / `DiscoveryResponse`：节点发现相关消息（可选）。
- `Data`：通用数据消息，携带业务负载。
- `Disconnect`：断开连接，用于通知对方清理资源。
- `Error`：错误消息，包含错误代码与描述。
- `Ack`：确认消息，用于确认接收并提升 UDP 可靠性。
- `Retransmit`：请求重传，用于在丢包场景下触发重发。

## 消息结构（`Message`）

消息为 JSON 格式，推荐字段如下（与当前实现保持一致）：

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

字段说明：
- `id`：消息唯一标识（UUID）。
- `message_type`：消息类型，见上文枚举。
- `timestamp`：消息产生时间戳（秒）。
- `payload`：消息主体，自由 JSON。
- `sender_addr`：发送方地址（字符串形式的 `SocketAddr`）。
- `sequence_number`：消息序列号，用于去重和确认匹配。
- `requires_ack`：是否需要对方返回 `Ack` 确认。
- `ack_for`：当本条为 `Ack` 时，指向被确认消息的序列号（数字）。

## 握手流程（带 ACK）

```text
Client                            Server
  | -- HandshakeRequest (requires_ack=true, seq=1) -->
  | <-- Ack (ack_for=1) -----------------------------
  | <-- HandshakeResponse (seq=2, requires_ack=true) -
  | -- Ack (ack_for=2) ----------------------------->
  |                [进入已认证状态]
```

说明：
- 客户端发起 `HandshakeRequest` 时可开启 `requires_ack`，服务器会返回 `Ack`。
- 服务器的 `HandshakeResponse` 同样可以要求 `Ack`，客户端应返回确认。
- 若在超时时间内未收到 `Ack`，可触发 `Retransmit`（见可靠性章节）。

## 心跳与数据传输

- `Ping` / `Pong`：用于健康检查与 RTT 测量，双方均可发起。
- `Data`：承载业务数据，可根据需要设置 `requires_ack`，以确保重要载荷的可靠送达。

## 错误与断开

- `Error`：用于传达解析失败、权限不足、消息非法等错误。
- `Disconnect`：用于显式断开，服务器接收后清理对应对等节点与连接状态。

## 序列号与幂等性建议

- 对需要可靠性的消息分配递增 `sequence_number`，便于去重与确认。
- 接收侧维护近期序列号缓存（窗口），忽略重复的消息处理以保持幂等。