# 服务器机制

本文档描述服务器在 UDP 场景下的主循环、消息处理与后台任务机制。

## 初始化

- 从配置（`Config`）中读取监听地址与最大连接数：
  - `listen_address`（如 `127.0.0.1:8080`）
  - `max_connections`
- 绑定 `UdpSocket`：在日志中打印 `UDP网络管理器已绑定到 <addr>`。

## 主循环（接收数据包）

1. 调用 `recv_from` 接收 UDP 数据：获取 `(buffer, source_addr)`。
2. 解析为 `Message`：包括 `message_type`、`payload`、`sequence_number` 等。
3. 通过地址获取/创建 `Connection` 与 `Peer`（基于 `SocketAddr` 索引）。
4. 分发到 `handle_message(message)` 进行具体处理。
5. 错误与异常：记录日志（`warn/error`），并在必要时清理状态。

## 消息处理（`handle_message`）

- 通用：若 `requires_ack = true`，先行发送 `Ack`。
- `HandshakeRequest`：校验与登记节点信息，返回 `HandshakeResponse`。
- `HandshakeResponse`：更新状态至“已认证”，可开始正常通信。
- `Ping`：返回 `Pong`。
- `Data`：按需处理 `payload`，可选择返回业务确认（或仅 ACK）。
- `DiscoveryRequest/Response`：节点发现相关流程（可选/规划中）。
- `Disconnect`：标记对等为断开状态，进入清理流程。
- `Error`：记录并按需上报或回复。
- `Retransmit`：根据序列号查询并重发或回复错误。

## 后台任务

- 心跳任务（`heartbeat_task`）：周期性向已认证节点发送健康检查；日志显示“发送心跳给 X 个节点”。
- 清理任务（`cleanup_task`）：清理长时间未活动或断开的对等节点；日志显示“执行对等节点清理任务”。
- 统计任务（`stats_task`）：周期统计当前节点数量与状态（总数/已认证/连接中）。
- 组合运行：`tokio::join!(heartbeat_task, cleanup_task, stats_task)`；
  - 注意：示例代码会出现 `unused_must_use` 警告，生产中应显式处理每个任务的结果。

## 错误处理与日志

- 建议使用命令行参数直接设置日志级别（优先级更高）：例如 `--INFO`、`--DEBUG`、`--TRACE`。未指定时可使用环境变量 `RUST_LOG`。
- 典型错误：`接收UDP数据包失败`，多见于客户端断开后仍在接收循环时；可降低日志级别或改为 `debug` 级别打印。
- 绑定失败：端口占用或权限不足；修改端口或调整权限。

## 优雅关闭

- 可通过控制台中断（如 `Ctrl+C`）触发退出；
- 清理资源并打印退出日志（进程退出码可能显示 `STATUS_CONTROL_C_EXIT`）。