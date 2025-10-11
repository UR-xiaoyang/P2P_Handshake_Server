# 节点发现机制

本机制用于在本地网络或指定端口范围内发现其他可用节点，以建立更丰富的 P2P 拓扑。

## 配置项

`config.json` 示例：

```json
{
  "enable_discovery": true,
  "discovery_port_range": [8081, 8090]
}
```

说明：
- `enable_discovery`：是否启用节点发现。
- `discovery_port_range`：在该端口范围尝试探测或广播。

## 发现思路（建议实现）

1. 主动探测：
   - 在 `discovery_port_range` 中向候选端口发送 `DiscoveryRequest`。
   - 收到 `DiscoveryResponse` 后记录节点信息，并进行握手。

2. 广播/组播（可选）：
   - 在本地子网内以广播或组播形式发送 `DiscoveryRequest`（需要网络环境支持）。
   - 监听组播地址，接收其他节点的发现响应。

3. 响应与速率限制：
   - 节点收到 `DiscoveryRequest` 后，返回 `DiscoveryResponse`，附带自身 `NodeInfo`。
   - 对请求施加速率限制，避免广播风暴。

## 与握手的关系

- 发现仅定位潜在节点；实际通信前仍需执行握手（认证）。
- 建议在成功发现后立刻发送 `HandshakeRequest`（可开启 `requires_ack`）。

## 状态与清理

- 对于长时间未响应或不可达的候选地址，定期清理以降低内存与时间消耗。

## 当前实现说明

- 配置已预留发现相关字段；具体广播/组播与端口扫描策略可按部署环境与性能目标逐步落地。