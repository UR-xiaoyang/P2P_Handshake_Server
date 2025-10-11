# P2P握手服务器（UDP版）

一个用Rust编写的高性能P2P网络握手服务器，采用UDP无连接传输，支持节点发现、消息路由和连接管理。

## 功能特性

- 🚀 **高性能异步网络处理** - 基于Tokio异步运行时
- 🤝 **P2P握手协议** - 完整的节点认证和握手流程
- 🔍 **节点发现** - 自动发现和连接网络中的其他节点
- 📡 **消息路由** - 智能消息转发和路由机制
- 🔗 **连接池管理** - 高效的连接生命周期管理
- ⚙️ **配置文件支持** - 灵活的JSON配置
- 📊 **完整日志记录** - 详细的运行状态监控
- 🛡️ **错误处理** - 健壮的错误恢复机制

- 📶 **UDP无连接传输** - 更低延迟，更适合P2P场景
- ✅ **可靠性增强** - 支持ACK确认与重传、序列号
- 🧭 **地址驱动的对等管理** - 基于`SocketAddr`的节点索引

## UDP改造概览

本项目已从TCP迁移到UDP，核心改动如下：

- `network.rs`：使用`UdpSocket`实现收发、维护已知对等地址、支持直接`send_to`与`receive_from`
- `protocol.rs`：新增`Ack`与`Retransmit`消息类型，消息增加`sequence_number`与`requires_ack/ack_for`字段
- `peer.rs`：增加基于地址的对等节点索引与查找，适配无连接特性
- `server.rs`：主循环由“接受连接”改为“接收数据包”，并在需要时自动回复ACK
- `examples/simple_client.rs`：客户端示例改为UDP实现，演示握手、数据、Ping/Pong与断开

## 快速开始

### 安装依赖

确保你已经安装了Rust（推荐使用最新稳定版）：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 编译项目

```bash
git clone <repository-url>
cd P2P_Handshake_Server
cargo build --release
```

### 运行服务器

使用默认配置运行：

```bash
cargo run --bin p2p_server
```

使用自定义配置：

```bash
cargo run --bin p2p_server -- --config config.json --address 127.0.0.1:8080
```

### 运行客户端示例

```bash
cargo run --example simple_client
```

建议设置日志级别以便观察协议交互：

```bash
# PowerShell
$env:RUST_LOG="debug"; cargo run --bin p2p_server
```

## 配置

服务器支持通过JSON配置文件进行配置。示例配置文件 `config.json`：

```json
{
  "listen_address": "127.0.0.1:8080",
  "max_connections": 100,
  "heartbeat_interval": 30,
  "connection_timeout": 60,
  "discovery_port_range": [8081, 8090],
  "enable_discovery": true
}
```

### 配置参数说明

- `listen_address`: 服务器监听地址和端口
- `max_connections`: 最大并发连接数
- `heartbeat_interval`: 心跳间隔（秒）
- `connection_timeout`: 连接超时时间（秒）
- `discovery_port_range`: 节点发现端口范围
- `enable_discovery`: 是否启用节点发现功能

## 命令行参数

```bash
p2p_server [OPTIONS]

OPTIONS:
    -a, --address <ADDRESS>           服务器监听地址 [default: 127.0.0.1:8080]
    -m, --max-connections <NUMBER>    最大连接数 [default: 100]
    -c, --config <FILE>              配置文件路径
    -h, --help                       显示帮助信息
```

## API文档

### 协议消息类型

服务器支持以下消息类型：

- `HandshakeRequest` - 握手请求
- `HandshakeResponse` - 握手响应
- `Ping` - 心跳包
- `Pong` - 心跳响应
- `DiscoveryRequest` - 节点发现请求
- `DiscoveryResponse` - 节点发现响应
- `Data` - 数据传输
- `Error` - 错误消息
- `Disconnect` - 断开连接
- `Ack` - 确认消息（用于提升UDP可靠性）
- `Retransmit` - 请求重传（在丢包场景下触发）

### 消息格式

所有消息都使用JSON格式，包含以下字段：

```json
{
  "id": "uuid",
  "message_type": "MessageType",
  "timestamp": 1234567890,
  "payload": {},
  "sequence_number": 1,
  "requires_ack": false,
  "ack_for": null
}
```

说明：
- `sequence_number`：消息序列号，用于去重与重传识别
- `requires_ack`：是否需要ACK确认
- `ack_for`：当为ACK消息时，指向被确认消息的序列号

### 握手流程

1. 客户端使用UDP向服务器发送 `HandshakeRequest`（可设置`requires_ack=true`）
2. 服务器解析请求并返回 `HandshakeResponse`，同时发送 `Ack`
3. 客户端收到 `HandshakeResponse` 与 `Ack` 后进入认证状态
4. 后续数据、Ping/Pong等消息可按需要求ACK；若未确认可触发`Retransmit`

## 开发

### 项目结构

```
src/
├── main.rs          # 主程序入口
├── lib.rs           # 库入口
├── config.rs        # 配置管理
├── network.rs       # 网络连接管理
├── peer.rs          # 对等节点管理
├── protocol.rs      # 通信协议定义
├── router.rs        # 消息路由
└── server.rs        # 主服务器逻辑

examples/
└── simple_client.rs # 客户端示例

config.json          # 示例配置文件
```

### 运行测试

```bash
cargo test
```

### 生成文档

```bash
cargo doc --open
```

## 使用示例

### 作为库使用

在你的 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
p2p_handshake_server = { path = "path/to/P2P_Handshake_Server" }
tokio = { version = "1.0", features = ["full"] }
```

示例代码：

```rust
use p2p_handshake_server::{Config, P2PServer};
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 创建配置
    let config = Config::new(
        "127.0.0.1:8080".parse::<SocketAddr>()?,
        50
    );
    
    // 创建并启动服务器
    let mut server = P2PServer::new(config).await?;
    server.run().await?;
    
    Ok(())
}
```

### 连接到其他节点

```rust
// 主动连接到其他节点
server.connect_to_peer("127.0.0.1:8081".parse()?).await?;
```

### 获取服务器统计信息

```rust
let stats = server.get_stats().await;
println!("连接的节点数: {}", stats.peer_stats.total_peers);
```

## 性能特性

- **异步I/O**: 基于Tokio的高性能异步网络处理
- **零拷贝**: 高效的消息序列化和传输
- **连接复用**: 智能的连接池管理
- **内存安全**: Rust的内存安全保证
- **并发处理**: 支持大量并发连接

## 安全考虑

- 消息大小限制（默认1MB）
- 连接数限制
- 超时机制
- 错误恢复
- 资源清理

## 故障排除

### 常见问题

1. **端口被占用**
   ```
   Error: 绑定地址 127.0.0.1:8080 失败
   ```
   解决方案：更改监听端口或停止占用端口的程序

2. **连接超时**
   ```
   Error: 连接到 x.x.x.x:xxxx 失败
   ```
   解决方案：检查网络连接和目标地址是否正确

3. **握手失败**
   ```
   Error: 握手失败: 节点ID xxx 已存在
   ```
   解决方案：确保每个节点使用唯一的ID

4. **接收UDP数据失败**
   ```
   ERROR p2p_server::server] 接收UDP数据包失败: 接收UDP数据失败
   ```
   说明：常见于客户端断开后，服务器仍在接收循环中；不影响整体功能。可通过降低日志级别或在接收失败处放宽日志级别进行优化。

## 迁移指南（TCP → UDP）

- 不再建立`TcpStream`连接，改为使用`UdpSocket`的`send_to/recv_from`
- 增加ACK与重传机制以提升可靠性（`Ack`/`Retransmit`，`requires_ack`与`sequence_number`）
- 节点管理依赖`SocketAddr`，同一地址即同一对等节点
- 主动连接改为发送握手请求包（`connect_to_peer`内部直接发送`HandshakeRequest`）
- 客户端在示例中展示：握手→发送数据→Ping/Pong→断开

### 日志级别

设置环境变量来控制日志级别：

```bash
export RUST_LOG=debug
cargo run --bin p2p_server
```

可用的日志级别：`error`, `warn`, `info`, `debug`, `trace`

## 贡献

欢迎提交Issue和Pull Request！

## 许可证

本项目采用MIT许可证。详见 [LICENSE](LICENSE) 文件。