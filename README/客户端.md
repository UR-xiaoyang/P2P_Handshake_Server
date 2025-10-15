# 客户端机制

本文档描述客户端在 UDP 场景下的典型工作流与实现要点，配合 `examples/simple_client.rs` 示例。

## 工作流

1. 创建并绑定 `UdpSocket`（系统选择本地临时端口）。
2. 准备服务器地址（如 `127.0.0.1:8080`）。
3. 发送 `HandshakeRequest`（建议设置 `requires_ack = true`）：
   - 等待 `Ack(ack_for=seq)` 与 `HandshakeResponse`。
4. 若握手成功，发送 `Data` 消息（可按需请求 ACK）。
5. 发送 `Ping`，接收 `Pong`，评估延迟与连通性。
6. 发送 `Disconnect`，完成优雅断开。

## 关键实现点

- `sequence_number`：为需要可靠性的消息分配序列号，便于确认与去重。
- `requires_ack`：对于握手、重要数据与控制消息建议开启。
- 接收循环：在发送后进入等待/接收逻辑，匹配消息类型与 `ack_for`。
- 超时与重试：若未在超时内收到关键回复，可重试或发送 `Retransmit`。

## 示例载荷

```json
{
  "message": "Hello from UDP client!",
  "timestamp": 1760000000
}
```

## 日志与问题排查

- 调试：建议在服务器端通过命令行指定日志级别观察交互细节，例如：`cargo run --bin p2p_server -- --DEBUG`。若未使用 CLI 指定，也可用环境变量：`RUST_LOG=debug`。
- 常见错误：
  - `远程主机强迫关闭了一个现有的连接 (10054)`：在 UDP 场景下通常表示对端不可达或接收端口变化，重试或重新握手。
  - `接收UDP数据包失败`：多见于服务器端在客户端断开后仍接收；可降低日志级别或忽略。