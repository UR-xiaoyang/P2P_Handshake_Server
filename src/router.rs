use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use log::{info, warn, debug, error};
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::protocol::{Message, MessageType};
use crate::peer::{PeerManager, Peer};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingTable {
    /// 节点ID到下一跳节点的映射
    routes: HashMap<Uuid, Uuid>,
    /// 节点ID到距离的映射
    distances: HashMap<Uuid, u32>,
}

impl RoutingTable {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
            distances: HashMap::new(),
        }
    }
    
    /// 添加路由条目
    pub fn add_route(&mut self, destination: Uuid, next_hop: Uuid, distance: u32) {
        // 只有当新路由距离更短时才更新
        if let Some(&existing_distance) = self.distances.get(&destination) {
            if distance >= existing_distance {
                return;
            }
        }
        
        self.routes.insert(destination, next_hop);
        self.distances.insert(destination, distance);
        
        debug!("添加路由: {} -> {} (距离: {})", destination, next_hop, distance);
    }
    
    /// 获取到目标节点的下一跳
    pub fn get_next_hop(&self, destination: &Uuid) -> Option<Uuid> {
        self.routes.get(destination).copied()
    }
    
    /// 获取到目标节点的距离
    pub fn get_distance(&self, destination: &Uuid) -> Option<u32> {
        self.distances.get(destination).copied()
    }
    
    /// 移除路由条目
    pub fn remove_route(&mut self, destination: &Uuid) {
        self.routes.remove(destination);
        self.distances.remove(destination);
        debug!("移除路由: {}", destination);
    }
    
    /// 移除通过特定下一跳的所有路由
    pub fn remove_routes_via(&mut self, next_hop: &Uuid) {
        let to_remove: Vec<Uuid> = self.routes
            .iter()
            .filter(|(_, &hop)| hop == *next_hop)
            .map(|(&dest, _)| dest)
            .collect();
        
        for dest in to_remove {
            self.remove_route(&dest);
        }
    }
    
    /// 获取所有路由条目
    pub fn get_all_routes(&self) -> Vec<(Uuid, Uuid, u32)> {
        self.routes
            .iter()
            .map(|(&dest, &next_hop)| {
                let distance = self.distances.get(&dest).copied().unwrap_or(u32::MAX);
                (dest, next_hop, distance)
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutedMessage {
    pub original_message: Message,
    pub source_node: Uuid,
    pub destination_node: Uuid,
    pub hop_count: u32,
    pub max_hops: u32,
    pub route_id: Uuid,
}

impl RoutedMessage {
    pub fn new(
        message: Message,
        source: Uuid,
        destination: Uuid,
        max_hops: u32,
    ) -> Self {
        Self {
            original_message: message,
            source_node: source,
            destination_node: destination,
            hop_count: 0,
            max_hops,
            route_id: Uuid::new_v4(),
        }
    }
    
    pub fn increment_hop(&mut self) -> bool {
        self.hop_count += 1;
        self.hop_count <= self.max_hops
    }
    
    pub fn to_message(&self) -> Message {
        let payload = serde_json::to_value(self).unwrap();
        Message::new(MessageType::Data, payload)
    }
    
    pub fn from_message(message: &Message) -> Result<Self> {
        if message.message_type != MessageType::Data {
            return Err(anyhow::anyhow!("不是数据消息"));
        }
        
        let routed_message: RoutedMessage = serde_json::from_value(message.payload.clone())?;
        Ok(routed_message)
    }
}

pub struct MessageRouter {
    routing_table: Arc<RwLock<RoutingTable>>,
    local_node_id: Uuid,
    peer_manager: Arc<PeerManager>,
    /// 消息缓存，防止重复转发
    message_cache: Arc<RwLock<HashMap<Uuid, std::time::Instant>>>,
    /// 缓存清理间隔
    cache_cleanup_interval: std::time::Duration,
}

impl MessageRouter {
    pub fn new(
        local_node_id: Uuid,
        peer_manager: Arc<PeerManager>,
    ) -> Self {
        Self {
            routing_table: Arc::new(RwLock::new(RoutingTable::new())),
            local_node_id,
            peer_manager,
            message_cache: Arc::new(RwLock::new(HashMap::new())),
            cache_cleanup_interval: std::time::Duration::from_secs(300), // 5分钟
        }
    }
    
    /// 路由消息到目标节点
    pub async fn route_message(
        &self,
        message: Message,
        destination: Uuid,
        max_hops: u32,
    ) -> Result<()> {
        // 如果目标是本地节点，直接处理
        if destination == self.local_node_id {
            return self.handle_local_message(message).await;
        }
        
        let routed_message = RoutedMessage::new(
            message,
            self.local_node_id,
            destination,
            max_hops,
        );
        
        self.forward_message(routed_message).await
    }
    
    /// 转发路由消息
    pub async fn forward_message(&self, mut routed_message: RoutedMessage) -> Result<()> {
        // 检查是否已经处理过这个消息
        if self.is_message_cached(&routed_message.route_id).await {
            debug!("消息 {} 已经处理过，跳过", routed_message.route_id);
            return Ok(());
        }
        
        // 缓存消息ID
        self.cache_message_id(routed_message.route_id).await;
        
        // 检查跳数限制
        if !routed_message.increment_hop() {
            warn!("消息 {} 达到最大跳数限制", routed_message.route_id);
            return Err(anyhow::anyhow!("达到最大跳数限制"));
        }
        
        // 如果目标是本地节点，处理消息
        if routed_message.destination_node == self.local_node_id {
            return self.handle_local_message(routed_message.original_message).await;
        }
        
        // 查找下一跳
        let next_hop = {
            let routing_table = self.routing_table.read().await;
            routing_table.get_next_hop(&routed_message.destination_node)
        };
        
        match next_hop {
            Some(next_hop_id) => {
                // 找到下一跳，转发消息
                if let Some(peer) = self.peer_manager.get_peer(&next_hop_id).await {
                    let message = routed_message.to_message();
                    peer.read().await.send_message(&message).await?;
                    
                    debug!(
                        "转发消息 {} 到下一跳 {} (目标: {})",
                        routed_message.route_id,
                        next_hop_id,
                        routed_message.destination_node
                    );
                } else {
                    // 下一跳节点不可达，移除路由并尝试广播
                    warn!("下一跳节点 {} 不可达，移除相关路由", next_hop_id);
                    self.routing_table.write().await.remove_routes_via(&next_hop_id);
                    
                    // 尝试广播到所有连接的节点
                    self.broadcast_message(routed_message).await?;
                }
            }
            None => {
                // 没有找到路由，广播到所有连接的节点
                debug!("没有找到到 {} 的路由，广播消息", routed_message.destination_node);
                self.broadcast_message(routed_message).await?;
            }
        }
        
        Ok(())
    }
    
    /// 广播消息到所有连接的节点
    async fn broadcast_message(&self, routed_message: RoutedMessage) -> Result<()> {
        let peers = self.peer_manager.get_authenticated_peers().await;
        let message = routed_message.to_message();
        
        let mut success_count = 0;
        let mut error_count = 0;
        
        for peer in peers {
            let peer_id = peer.read().await.id;
            
            // 不要发送回源节点
            if peer_id == routed_message.source_node {
                continue;
            }
            
            match peer.read().await.send_message(&message).await {
                Ok(_) => {
                    success_count += 1;
                    debug!("广播消息到节点 {}", peer_id);
                }
                Err(e) => {
                    error_count += 1;
                    warn!("广播消息到节点 {} 失败: {}", peer_id, e);
                }
            }
        }
        
        info!(
            "广播消息 {} 完成: 成功 {}, 失败 {}",
            routed_message.route_id,
            success_count,
            error_count
        );
        
        Ok(())
    }
    
    /// 处理本地消息
    async fn handle_local_message(&self, message: Message) -> Result<()> {
        info!("处理本地消息: {:?}", message.message_type);
        
        // 这里可以根据消息类型进行不同的处理
        match message.message_type {
            MessageType::Data => {
                // 处理数据消息
                debug!("接收到数据消息: {:?}", message.payload);
            }
            _ => {
                debug!("接收到其他类型消息: {:?}", message.message_type);
            }
        }
        
        Ok(())
    }
    
    /// 更新路由表
    pub async fn update_routing_table(&self, node_id: Uuid, next_hop: Uuid, distance: u32) {
        self.routing_table.write().await.add_route(node_id, next_hop, distance);
    }
    
    /// 移除节点的路由
    pub async fn remove_node_routes(&self, node_id: &Uuid) {
        let mut routing_table = self.routing_table.write().await;
        routing_table.remove_route(node_id);
        routing_table.remove_routes_via(node_id);
    }
    
    /// 获取路由表快照
    pub async fn get_routing_table_snapshot(&self) -> Vec<(Uuid, Uuid, u32)> {
        self.routing_table.read().await.get_all_routes()
    }
    
    /// 检查消息是否已缓存
    async fn is_message_cached(&self, message_id: &Uuid) -> bool {
        self.message_cache.read().await.contains_key(message_id)
    }
    
    /// 缓存消息ID
    async fn cache_message_id(&self, message_id: Uuid) {
        self.message_cache.write().await.insert(message_id, std::time::Instant::now());
    }
    
    /// 启动缓存清理任务
    pub fn start_cache_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let message_cache = self.message_cache.clone();
        let cleanup_interval = self.cache_cleanup_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            
            loop {
                interval.tick().await;
                
                let now = std::time::Instant::now();
                let mut cache = message_cache.write().await;
                
                // 移除超过5分钟的缓存条目
                cache.retain(|_, &mut timestamp| {
                    now.duration_since(timestamp) < std::time::Duration::from_secs(300)
                });
                
                debug!("清理消息缓存，当前缓存大小: {}", cache.len());
            }
        })
    }
    
    /// 处理路由发现
    pub async fn handle_route_discovery(&self, source: Uuid, target: Uuid) -> Result<()> {
        // 简单的路由发现：如果我们知道目标节点，返回路由信息
        let routing_table = self.routing_table.read().await;
        
        if let Some(next_hop) = routing_table.get_next_hop(&target) {
            if let Some(distance) = routing_table.get_distance(&target) {
                // 发送路由响应给源节点
                let route_info = serde_json::json!({
                    "target": target,
                    "next_hop": next_hop,
                    "distance": distance + 1
                });
                
                let response = Message::new(MessageType::Data, route_info);
                self.route_message(response, source, 10).await?;
                
                debug!("发送路由信息给 {}: {} -> {} (距离: {})", source, target, next_hop, distance + 1);
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::{PeerManager, PeerStatus};
    use crate::network::Connection;
    use crate::protocol::{NodeInfo, Message, MessageType};
    use tokio::net::UdpSocket;
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};
    
    #[test]
    fn test_routing_table() {
        let mut table = RoutingTable::new();
        let dest = Uuid::new_v4();
        let next_hop = Uuid::new_v4();
        
        table.add_route(dest, next_hop, 1);
        
        assert_eq!(table.get_next_hop(&dest), Some(next_hop));
        assert_eq!(table.get_distance(&dest), Some(1));
        
        table.remove_route(&dest);
        assert_eq!(table.get_next_hop(&dest), None);
    }
    
    #[test]
    fn test_routed_message() {
        let message = Message::ping();
        let source = Uuid::new_v4();
        let dest = Uuid::new_v4();
        
        let mut routed = RoutedMessage::new(message, source, dest, 5);
        
        assert_eq!(routed.hop_count, 0);
        assert!(routed.increment_hop());
        assert_eq!(routed.hop_count, 1);
    }

    #[tokio::test]
    async fn test_forward_via_next_hop() {
        // 建立两个UDP套接字，模拟本地与下一跳对端
        let sock_local = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let local_addr = sock_local.local_addr().unwrap();
        let sock_next = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let next_addr = sock_next.local_addr().unwrap();

        // 创建到下一跳的连接（使用本地socket向 next_addr 发送）
        let conn = Arc::new(Connection::new(sock_local.clone(), next_addr, local_addr));

        let local_info = NodeInfo::new("local_test".to_string(), local_addr, "testnet".to_string());
        let peer_manager = Arc::new(PeerManager::new(local_info.clone(), 10));

        // 加入一个已认证的下一跳节点
        let peer = peer_manager.add_peer(conn.clone()).await.unwrap();
        peer.write().await.update_status(PeerStatus::Authenticated);
        let next_hop_id = peer.read().await.id;

        let router = MessageRouter::new(local_info.id, peer_manager.clone());

        // 为随机目的地添加路由，下一跳为已加入的peer
        let dest = Uuid::new_v4();
        router.update_routing_table(dest, next_hop_id, 1).await;

        // 发送路由数据消息，应成功通过下一跳发送
        let msg = Message::data(serde_json::json!({"k":"v"}));
        let res = router.route_message(msg, dest, 10).await;
        assert!(res.is_ok());

        // 在下一跳socket上接收并断言内容
        let mut buf = vec![0u8; 65536];
        let (len, _from) = timeout(Duration::from_millis(300), sock_next.recv_from(&mut buf)).await.unwrap().unwrap();
        buf.truncate(len);
        let received: Message = serde_json::from_slice(&buf).unwrap();
        assert_eq!(received.message_type, MessageType::Data);
        let routed = RoutedMessage::from_message(&received).unwrap();
        assert_eq!(routed.destination_node, dest);
        assert_eq!(routed.source_node, local_info.id);
    }

    #[tokio::test]
    async fn test_broadcast_when_no_route() {
        // 一个发送socket，两个不同的对端地址
        let sock_local = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let local_addr = sock_local.local_addr().unwrap();
        let sock_peer1 = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr1 = sock_peer1.local_addr().unwrap();
        let sock_peer2 = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr2 = sock_peer2.local_addr().unwrap();

        let conn1 = Arc::new(Connection::new(sock_local.clone(), addr1, local_addr));
        let conn2 = Arc::new(Connection::new(sock_local.clone(), addr2, local_addr));

        let local_info = NodeInfo::new("local_test".to_string(), local_addr, "testnet".to_string());
        let peer_manager = Arc::new(PeerManager::new(local_info.clone(), 10));

        let p1 = peer_manager.add_peer(conn1.clone()).await.unwrap();
        p1.write().await.update_status(PeerStatus::Authenticated);
        let p2 = peer_manager.add_peer(conn2.clone()).await.unwrap();
        p2.write().await.update_status(PeerStatus::Authenticated);

        let router = MessageRouter::new(local_info.id, peer_manager.clone());

        // 随机目的地没有路由，触发广播到所有已认证节点
        let dest = Uuid::new_v4();
        let msg = Message::data(serde_json::json!({"broadcast":"yes"}));
        let res = router.route_message(msg, dest, 10).await;
        assert!(res.is_ok());

        // 两个对端都应接收到消息
        let mut buf1 = vec![0u8; 65536];
        let (len1, _from1) = timeout(Duration::from_millis(300), sock_peer1.recv_from(&mut buf1)).await.unwrap().unwrap();
        buf1.truncate(len1);
        let recv1: Message = serde_json::from_slice(&buf1).unwrap();
        assert_eq!(recv1.message_type, MessageType::Data);
        let routed1 = RoutedMessage::from_message(&recv1).unwrap();
        assert_eq!(routed1.destination_node, dest);

        let mut buf2 = vec![0u8; 65536];
        let (len2, _from2) = timeout(Duration::from_millis(300), sock_peer2.recv_from(&mut buf2)).await.unwrap().unwrap();
        buf2.truncate(len2);
        let recv2: Message = serde_json::from_slice(&buf2).unwrap();
        assert_eq!(recv2.message_type, MessageType::Data);
        let routed2 = RoutedMessage::from_message(&recv2).unwrap();
        assert_eq!(routed2.destination_node, dest);
    }

    #[tokio::test]
    async fn test_unreachable_next_hop_removes_route_and_broadcasts() {
        // 一个发送socket和一个已认证peer，用于接收广播
        let sock_local = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let local_addr = sock_local.local_addr().unwrap();
        let sock_peer = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let addr_peer = sock_peer.local_addr().unwrap();

        let conn_peer = Arc::new(Connection::new(sock_local.clone(), addr_peer, local_addr));

        let local_info = NodeInfo::new("local_test".to_string(), local_addr, "testnet".to_string());
        let peer_manager = Arc::new(PeerManager::new(local_info.clone(), 10));

        let p = peer_manager.add_peer(conn_peer.clone()).await.unwrap();
        p.write().await.update_status(PeerStatus::Authenticated);

        let router = MessageRouter::new(local_info.id, peer_manager.clone());

        // 为目的地添加一个不可达的下一跳（未加入到PeerManager），随后应移除此路由并广播
        let dest = Uuid::new_v4();
        let unreachable_next_hop = Uuid::new_v4();
        router.update_routing_table(dest, unreachable_next_hop, 1).await;

        let msg = Message::data(serde_json::json!({"payload":"x"}));
        let res = router.route_message(msg, dest, 5).await;
        assert!(res.is_ok());

        // 应广播到已认证peer
        let mut buf = vec![0u8; 65536];
        let (len, _from) = timeout(Duration::from_millis(300), sock_peer.recv_from(&mut buf)).await.unwrap().unwrap();
        buf.truncate(len);
        let received: Message = serde_json::from_slice(&buf).unwrap();
        assert_eq!(received.message_type, MessageType::Data);
        let routed = RoutedMessage::from_message(&received).unwrap();
        assert_eq!(routed.destination_node, dest);

        // 路由表快照中不应再存在该目的地的条目
        let snapshot = router.get_routing_table_snapshot().await;
        let still_exists = snapshot.iter().any(|(d, _, _)| *d == dest);
        assert!(!still_exists);
    }
}