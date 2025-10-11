# 总览

本项目是一个使用 Rust 构建的 P2P 握手服务器，已从 TCP 迁移到 UDP 无连接传输，以获得更低的延迟与更适合 P2P 的通信模式。系统以模块化方式组织，核心组件如下：

- `network.rs`：网络层，使用 `UdpSocket` 完成消息的发送与接收；维护已知的对等地址映射。
- `protocol.rs`：协议层，定义消息类型与消息结构（包含 ACK/重传等提高 UDP 可靠性的字段）。
- `peer.rs`：对等节点管理，基于 `SocketAddr` 的索引实现无连接场景下的节点查找与生命周期管理。
- `server.rs`：服务器主循环，从 UDP 套接字读取数据包、解析消息并分发处理（必要时自动发送 ACK）。
- `examples/simple_client.rs`：示例客户端，演示握手、发送数据、Ping/Pong 与断开流程。
- `router.rs`：路由层（规划中的模块），为多跳消息与路由表维护提供接口，目前未与主流程集成。

## 设计动机（为何选择 UDP）

- 更低延迟：无需连接建立与保持，消息即发即到。
- 更适合 P2P：便于多播/广播与节点发现，减少连接维护开销。
- 可靠性可控：通过应用层 ACK 与重传机制在需要处增强可靠性。

## 数据流（高层）

1. 客户端通过 `UdpSocket::send_to` 向服务器监听地址发送消息（如握手请求）。
2. 服务器通过 `UdpSocket::recv_from` 接收数据包，解析为 `Message`。
3. 服务器通过地址映射获取/创建 `Peer`，再根据 `MessageType` 分发处理。
4. 若消息需要确认（`requires_ack = true`），服务器会回复 `Ack`；
5. 双方根据需要发送数据（`Data`）、心跳（`Ping/Pong`），以及断开（`Disconnect`）。

## 关键能力

- UDP 无连接传输，直接基于地址与数据包工作。
- 应用层可靠性增强：`Ack`、`Retransmit`、`sequence_number` 支持。
- 基于 `SocketAddr` 的对等节点索引，确保消息能正确关联到对等实体。
- 背景任务：心跳、清理与统计（Tokio 异步定时器与 `join!` 管理）。

## 运行与日志

建议在开发调试时开启详细日志：

```powershell
$env:RUST_LOG="debug"; cargo run --bin p2p_server
```

客户端示例：

```powershell
cargo run --example simple_client
```

日志中可观察到握手、ACK、数据、Ping/Pong 与断开等完整交互过程。