# 路由机制（规划中）

`router.rs` 提供了一系列路由相关的接口（如 `route_message`、`forward_message`、`broadcast_message` 等），用于未来在多节点多跳拓扑中进行消息转发与路由表维护。本文件描述该模块的目标与设计草案。

## 目标

- 支持消息在多跳网络中的转发与回传。
- 维护动态路由表，选择最优下一跳（延迟/可靠性/带宽综合考虑）。
- 提供消息去重缓存，减少环路与重复传播。

## 核心能力（接口草案）

- `route_message(message)`：根据目的节点选择下一跳或决定本地处理。
- `forward_message(routed_message)`：将消息转发到下一跳。
- `broadcast_message(routed_message)`：在一定范围内广播消息（带防重与 TTL）。
- `handle_local_message(message)`：当目的地为本节点时的处理逻辑。
- `update_routing_table(node_id, next_hop)`：更新路由表条目。
- `remove_node_routes(node_id)`：移除某节点相关路由。
- `get_routing_table_snapshot()`：观察当前路由状态。
- 缓存相关：`is_message_cached` / `cache_message_id` / 周期清理任务。

## 集成计划

- 在握手完成后，向路由层登记可达的邻居节点与链路质量。
- 数据消息在本地不可达时，委托路由层选择下一跳转发。
- 配合 ACK/重传机制，统计链路质量并动态调整路由。

## 现状与注意

- 当前模块尚未与主服务器流程集成，相关函数可能出现“未使用”的编译警告。
- 后续集成需评估性能与复杂度，谨慎引入广播与多跳逻辑，避免网络风暴。