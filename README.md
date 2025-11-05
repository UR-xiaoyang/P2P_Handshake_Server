# P2P握手服务器（UDP版）

一个用Rust编写的高性能P2P网络握手服务器，采用UDP无连接传输，支持节点发现、消息路由和连接管理。

## 功能特性

- 🚀 **高性能异步网络处理** - 基于Tokio异步运行时
- 🤝 **P2P握手协议** - 完整的节点认证和握手流程
- 🔍 **节点发现** - 自动发现和连接网络中的其他节点
- 📡 **消息路由** - 智能消息转发和路由机制
- 🔗 **连接池管理** - 高效的连接生命周期管理
- ⚙️ **配置文件支持** - 灵活的TOML配置
- 📊 **完整日志记录** - 详细的运行状态监控
- 🛡️ **错误处理** - 健壮的错误恢复机制

- 📶 **UDP无连接传输** - 更低延迟，更适合P2P场景
- ✅ **可靠性增强** - 支持ACK确认与重传、序列号
- 🧭 **地址驱动的对等管理** - 基于`SocketAddr`的节点索引

### 🆕 ICE增强功能

- 🔍 **NAT类型检测** - 自动识别完全锥形、受限锥形、端口受限锥形和对称NAT
- 🌐 **内置STUN服务器** - 可选的内置STUN服务，支持NAT穿透
- 🔄 **流量转发机制** - 为对称NAT客户端提供服务器中继转发
- 🎯 **智能端口预测** - 基于历史模式和机器学习的端口预测算法
- 📈 **频率调优** - 动态调整连接尝试频率以提高成功率
- 🔒 **安全控制** - 可配置的转发权限和速率限制

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
cargo run --bin p2p_server -- --config config.toml --address 127.0.0.1:8080
```

启用ICE增强功能：

```bash
# 启用内置STUN服务器
cargo run --bin p2p_server -- --STUN

# 启用流量转发
cargo run --bin p2p_server -- --relay

# 同时启用STUN和流量转发
cargo run --bin p2p_server -- --STUN --relay
```

### 运行客户端示例

```bash
cargo run --example simple_client
```

### 日志级别

建议优先使用命令行参数设置日志级别（优先级更高）：

```bash
cargo run --bin p2p_server -- --INFO
```

未指定 CLI 日志级别时，可通过环境变量控制：

```bash
export RUST_LOG=info
cargo run --bin p2p_server
```

可用的日志级别：`ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`

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

为了提供更灵活的配置方式，服务器支持通过命令行参数进行配置。这些参数会覆盖配置文件中的相应设置。

- `--config <PATH>`: 指定配置文件的路径。
- `--listen-address <ADDRESS>`: 设置服务器监听的IP地址和端口。
- `--max-connections <NUMBER>`: 设置最大客户端连接数。
- `--network-id <ID>`: 指定P2P网络的唯一标识符。
- `--heartbeat-interval <SECONDS>`: 设置心跳消息的发送频率（秒）。
- `--connection-timeout <SECONDS>`: 设置连接因不活动而超时的时长（秒）。
- `--enable-discovery <true|false>`: 启用或禁用节点发现功能。

#### 示例

```bash
# 使用指定的网络ID和监听地址启动服务器
$ cargo run -- --network-id "my-test-network" --listen-address "127.0.0.1:9000"

# 从配置文件加载配置，但覆盖最大连接数
$ cargo run -- --config config.json --max-connections 200
```

## 设计

```bash
p2p_server [OPTIONS]

OPTIONS:
    -a, --address <ADDRESS>           服务器监听地址 [default: 127.0.0.1:8080]
    -m, --max-connections <NUMBER>    最大连接数 [default: 100]
    -c, --config <FILE>               配置文件路径
        --TRACE                       设置日志级别为 TRACE（与下列日志级别互斥）
        --DEBUG                       设置日志级别为 DEBUG（互斥）
        --INFO                        设置日志级别为 INFO（互斥）
        --WARN                        设置日志级别为 WARN（互斥）
        --ERROR                       设置日志级别为 ERROR（互斥）
    -h, --help                        显示帮助信息
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

## 实操教程（一步上手）

以下以本机回环地址为例，带你从零启动并完成一次完整的握手与消息交互。

### 步骤 1：克隆并构建

```bash
git clone <repository-url>
cd P2P_Handshake_Server
cargo build --release
```

### 步骤 2：启动服务器

- 使用默认配置（监听 `127.0.0.1:8080`，最大连接数 100）：

```bash
cargo run --bin p2p_server
```

- 指定监听地址与连接数（未提供 `--config` 时生效）：

```bash
cargo run --bin p2p_server -- --address 127.0.0.1:8080 --max-connections 200
```

- 从配置文件启动（当传入 `--config` 时，配置文件优先于命令行地址与连接数）：

```bash
cargo run --bin p2p_server -- --config config.json
```

### 步骤 3：运行客户端示例

```bash
cargo run --example simple_client
```

客户端会依次发送握手、数据与 Ping，并打印收到的响应（如 `HandshakeResponse` 与 `Pong`）。

### 步骤 4：观察交互

- 服务器日志会显示握手、ACK、数据回显以及心跳统计等信息。
- 客户端会打印从服务器接收的各类响应与可能发生的超时提示。

## 配置优先级与覆盖规则

- 传入 `--config <file>` 时：程序从文件加载所有字段（`listen_address`、`max_connections`、`heartbeat_interval` 等），这些值在运行期生效。
- 未传入 `--config` 时：使用命令行的 `--address` 与 `--max-connections` 构建默认配置，其余字段使用代码默认值（心跳 30s、连接超时 60s、发现端口范围 `8081-8090`、启用发现）。

示例配置（`config.json`）：

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

## 协议交互示例（JSON）

握手请求（带 ACK）：

```json
{
  "id": "b5e2e2b2-...",
  "message_type": "HandshakeRequest",
  "timestamp": 1710000000,
  "payload": {
    "node_id": "client-uuid",
    "listen_addr": "127.0.0.1:9000",
    "capabilities": ["test"],
    "metadata": {"client_type": "udp_test_client"}
  },
  "sequence_number": 1,
  "requires_ack": true,
  "ack_for": null
}
```

对应 ACK：

```json
{
  "id": "c1f4a8f3-...",
  "message_type": "Ack",
  "timestamp": 1710000001,
  "payload": null,
  "sequence_number": 0,
  "requires_ack": false,
  "ack_for": 1
}
```

数据消息与回显响应：客户端发送 `Data`，服务器回 `{"echo": ..., "timestamp": ...}`。

## 日志与调试

- 建议优先通过命令行参数设置日志级别（如 `--INFO`、`--DEBUG`、`--TRACE`）；未指定时可使用环境变量 `RUST_LOG`（支持 `error | warn | info | debug | trace`）。
- 生产运行建议使用 `info`，调试时使用 `debug` 或 `trace`（可能较为冗长）。
- 定期统计输出：服务器每 5 分钟打印对等节点统计（已认证、连接中等）。

## 常见问题与排查

- 端口占用：修改 `--address` 端口或更新 `config.json`；在 Windows 上可用 `netstat -ano | findstr :8080` 查找占用进程。
- 防火墙拦截：确保操作系统防火墙允许 UDP 入站到监听端口；企业网络可能对广播/发现端口有限制。
- 收不到 ACK：检查网络丢包与 `requires_ack` 设置，必要时调整心跳与超时参数。
- 超时过多：在高丢包网络下，适当加大 `connection_timeout` 与重试策略；关注日志中的 Error 与 Warn。

## 使用 GitHub Actions 构建产物（下载与运行）

本仓库的 CI 会为多个平台产出可执行包。你可以在 GitHub 的 Actions 任务详情中下载构建产物（Artifacts），或在打标签（`v*`）后在 Releases 草稿中下载（路径为 `dist/**`）。

### 产物命名与内容
- Windows：`p2p_server-<target>-windows.zip`，解压后得到 `p2p_server-<target>-windows.exe`
- Linux：`p2p_server-<target>-linux.tar.gz`，解压后得到 `p2p_server-<target>-linux`
- macOS：`p2p_server-<target>-macos.tar.gz`，解压后得到 `p2p_server-<target>-macos`

示例目标标识：`x86_64-pc-windows-msvc`、`x86_64-unknown-linux-gnu`、`x86_64-apple-darwin`。

### Windows 运行

```powershell
# 解压
Expand-Archive -Path p2p_server-x86_64-pc-windows-msvc-windows.zip -DestinationPath .

# 运行（优先从配置文件读取）
./p2p_server-x86_64-pc-windows-msvc-windows.exe --config config.json

# 或使用命令行参数（未提供 --config 时生效）
./p2p_server-x86_64-pc-windows-msvc-windows.exe --address 127.0.0.1:8080 --max-connections 200

# 设置日志级别

# 优先使用 CLI 指定日志级别
./p2p_server-x86_64-unknown-linux-gnu-linux --INFO

# 若未指定 CLI 日志级别，可使用环境变量
RUST_LOG=info ./p2p_server-x86_64-unknown-linux-gnu-linux
```

可选：将可执行文件重命名为 `p2p_server.exe` 以便调用。

### Linux 运行

```bash
# 解压
tar -xzf p2p_server-x86_64-unknown-linux-gnu-linux.tar.gz

# 赋权（如需）
chmod +x p2p_server-x86_64-unknown-linux-gnu-linux

# 运行
./p2p_server-x86_64-unknown-linux-gnu-linux --config config.json

# 日志级别（优先使用 CLI 指定）
./p2p_server-x86_64-unknown-linux-gnu-linux --INFO
# 若未指定 CLI 日志级别，可使用环境变量
RUST_LOG=info ./p2p_server-x86_64-unknown-linux-gnu-linux
```

### macOS 运行

```bash
# 解压
tar -xzf p2p_server-x86_64-apple-darwin-macos.tar.gz

# 赋权（如需）
chmod +x p2p_server-x86_64-apple-darwin-macos

# 运行
./p2p_server-x86_64-apple-darwin-macos --config config.json

# 日志级别（优先使用 CLI 指定）
./p2p_server-x86_64-apple-darwin-macos --INFO
# 若未指定 CLI 日志级别，可使用环境变量
RUST_LOG=info ./p2p_server-x86_64-apple-darwin-macos
```

### 参数与配置优先级
- `--config <file>`：从文件加载所有配置（优先级最高）。
- 未提供 `--config` 时，命令行的 `--address` 与 `--max-connections` 生效，其余字段使用默认值（心跳 30s、超时 60s、发现端口范围 `8081-8090`、启用发现）。

### 校验与发布
- Release 任务会在 `dist/SHA256SUMS.txt` 中生成校验文件。
- Linux/macOS 校验：`sha256sum <artifact>`；Windows 校验：`certutil -hashfile <artifact> SHA256`。
- 构建发布为草稿（`draft: true`），可在 GitHub 上确认后正式发布。

### 注意事项
- 构建产物仅包含服务器二进制；示例客户端不打包。如需联调，请在源码仓库运行：`cargo run --example simple_client`。
- Windows 可能需要在防火墙中放行入站 UDP；Linux/macOS 需确认端口未被占用。

## 生产部署建议

- 使用 `cargo build --release` 构建并运行，以获得更佳性能。
- 将配置与日志级别分离至环境与文件，避免硬编码。
- 监控与滚动重启：结合系统服务或进程管理器（如 systemd、NSSM）进行守护与重启。

## 深入文档

- 中文文档：
  - `README/overview.md`（总体架构）
  - `README/protocol.md`（协议说明）
  - `README/discovery.md`（节点发现）
  - `README/routing.md`（消息路由）
  - `README/server.md`（服务器实现）
  - `README/client.md`（客户端示例）

- English Docs:
  - `README-EN/overview.md`
  - `README-EN/protocol.md`
  - `README-EN/discovery.md`
  - `README-EN/routing.md`
  - `README-EN/server.md`
  - `README-EN/client.md`

- 错误恢复
- 资源清理

- 更新日志：`CHANGELOG.md`（包含版本历史与变更说明）

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
export RUST_LOG=info
cargo run --bin p2p_server
```

可用的日志级别：`error`, `warn`, `info`, `debug`, `trace`

## 相关文档

- [ICE增强功能详细说明](ICE_ENHANCEMENT.md) - 详细的NAT穿透和流量转发功能文档
- [配置文件示例](config_example.toml) - 完整的配置文件示例，包含所有可用选项

## 许可证

本项目采用MIT许可证。
export RUST_LOG=debug
cargo run --bin p2p_server
```

可用的日志级别：`error`, `warn`, `info`, `debug`, `trace`

## 贡献

欢迎提交Issue和Pull Request！

## 许可证

本项目采用MIT许可证。详见 [LICENSE](LICENSE) 文件。