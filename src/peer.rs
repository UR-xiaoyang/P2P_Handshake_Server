use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use std::net::SocketAddr;
use log::{info, warn, debug};
use anyhow::Result;

use crate::network::Connection;
use crate::protocol::{NodeInfo, PeerInfo, Message, MessageType, HandshakeProtocol};

#[derive(Debug, Clone)]
pub enum PeerStatus {
    Connecting,
    Connected,
    Handshaking,
    Authenticated,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct Peer {
    pub id: Uuid,
    pub node_info: Option<NodeInfo>,
    pub connection: Arc<Connection>,
    pub status: PeerStatus,
    pub last_ping: Option<std::time::Instant>,
    pub created_at: std::time::Instant,
}

impl Peer {
    pub fn new(connection: Arc<Connection>) -> Self {
        Self {
            id: Uuid::new_v4(),
            node_info: None,
            connection,
            status: PeerStatus::Connecting,
            last_ping: None,
            created_at: std::time::Instant::now(),
        }
    }
    
    pub fn with_node_info(connection: Arc<Connection>, node_info: NodeInfo) -> Self {
        Self {
            id: node_info.id,
            node_info: Some(node_info),
            connection,
            status: PeerStatus::Authenticated,
            last_ping: None,
            created_at: std::time::Instant::now(),
        }
    }
    
    pub fn update_status(&mut self, status: PeerStatus) {
        debug!("节点 {} 状态更新: {:?} -> {:?}", self.id, self.status, status);
        self.status = status;
    }
    
    pub fn update_ping(&mut self) {
        self.last_ping = Some(std::time::Instant::now());
    }
    
    pub fn is_authenticated(&self) -> bool {
        matches!(self.status, PeerStatus::Authenticated)
    }
    
    pub fn is_connected(&self) -> bool {
        matches!(self.status, PeerStatus::Connected | PeerStatus::Authenticated)
    }
    
    pub fn addr(&self) -> SocketAddr {
        self.connection.peer_addr()
    }
    
    /// 发送消息给对等节点
    pub async fn send_message(&self, message: &Message) -> Result<()> {
        self.connection.send_message(message).await
    }
    
    /// 接收来自对等节点的消息
    pub async fn receive_message(&self) -> Result<Option<Message>> {
        self.connection.receive_message().await
    }
}

pub struct PeerManager {
    peers: Arc<RwLock<HashMap<Uuid, Arc<RwLock<Peer>>>>>,
    // UDP需要基于地址的索引
    peers_by_addr: Arc<RwLock<HashMap<SocketAddr, Arc<RwLock<Peer>>>>>,
    local_node_info: NodeInfo,
    max_connections: usize,
}

impl PeerManager {
    pub fn new(local_node_info: NodeInfo, max_connections: usize) -> Self {
        Self {
            peers: Arc::new(RwLock::new(HashMap::new())),
            peers_by_addr: Arc::new(RwLock::new(HashMap::new())),
            local_node_info,
            max_connections,
        }
    }
    
    /// 添加新的对等节点
    pub async fn add_peer(&self, connection: Arc<Connection>) -> Result<Arc<RwLock<Peer>>> {
        let peers_count = self.peers.read().await.len();
        if peers_count >= self.max_connections {
            return Err(anyhow::anyhow!("已达到最大连接数限制: {}", self.max_connections));
        }
        
        let peer = Arc::new(RwLock::new(Peer::new(connection)));
        let peer_id = peer.read().await.id;
        let peer_addr = peer.read().await.addr();
        
        // 同时维护两个索引
        self.peers.write().await.insert(peer_id, peer.clone());
        self.peers_by_addr.write().await.insert(peer_addr, peer.clone());
        
        info!("添加新的对等节点: {} ({})", peer_id, peer_addr);
        
        Ok(peer)
    }
    
    /// 移除对等节点
    pub async fn remove_peer(&self, peer_id: &Uuid) -> Option<Arc<RwLock<Peer>>> {
        let removed = self.peers.write().await.remove(peer_id);
        
        if let Some(ref peer) = removed {
            let peer_addr = peer.read().await.addr();
            self.peers_by_addr.write().await.remove(&peer_addr);
            info!("移除对等节点: {} ({})", peer_id, peer_addr);
        }
        
        removed
    }
    
    /// 获取对等节点
    pub async fn get_peer(&self, peer_id: &Uuid) -> Option<Arc<RwLock<Peer>>> {
        self.peers.read().await.get(peer_id).cloned()
    }
    
    /// 根据地址获取对等节点（UDP需要）
    pub async fn get_peer_by_addr(&self, addr: &SocketAddr) -> Option<Arc<RwLock<Peer>>> {
        self.peers_by_addr.read().await.get(addr).cloned()
    }
    
    /// 获取或创建基于地址的peer（UDP需要）
    pub async fn get_or_create_peer_by_addr(&self, connection: Arc<Connection>) -> Result<Arc<RwLock<Peer>>> {
        let addr = connection.peer_addr();
        
        // 先尝试获取现有的peer
        if let Some(peer) = self.get_peer_by_addr(&addr).await {
            return Ok(peer);
        }
        
        // 如果不存在，创建新的peer
        self.add_peer(connection).await
    }
    
    /// 获取所有对等节点
    pub async fn get_all_peers(&self) -> Vec<Arc<RwLock<Peer>>> {
        self.peers.read().await.values().cloned().collect()
    }
    
    /// 获取已认证的对等节点
    pub async fn get_authenticated_peers(&self) -> Vec<Arc<RwLock<Peer>>> {
        let peers = self.peers.read().await;
        let mut authenticated = Vec::new();
        
        for peer in peers.values() {
            if peer.read().await.is_authenticated() {
                authenticated.push(peer.clone());
            }
        }
        
        authenticated
    }
    
    /// 处理握手请求
    pub async fn handle_handshake_request(
        &self,
        peer: Arc<RwLock<Peer>>, 
        message: &Message,
    ) -> Result<()> {
        let node_info = HandshakeProtocol::validate_handshake_request(message)
            .map_err(|e| anyhow::anyhow!("握手请求验证失败: {}", e))?;
        
        let peer_addr = peer.read().await.addr();
        info!(
            "收到握手请求: 对端地址={}、节点名={}、节点ID={}、网络ID={}",
            peer_addr, node_info.name, node_info.id, node_info.network_id
        );

        // 检查网络ID是否匹配
        if node_info.network_id != self.local_node_info.network_id {
            let error_msg = format!("网络ID不匹配: 期望 {}，收到 {}", self.local_node_info.network_id, node_info.network_id);
            warn!("{}", error_msg);
            let error_response = Message::error(error_msg.clone());
            peer.read().await.send_message(&error_response).await?;
            return Err(anyhow::anyhow!(error_msg));
        }

        // 检查是否已经有相同ID的节点
        if self.peers.read().await.contains_key(&node_info.id) {
            let error_msg = format!("节点ID {} 已存在", node_info.id);
            let error_response = Message::error(error_msg.clone());
            peer.read().await.send_message(&error_response).await?;
            return Err(anyhow::anyhow!(error_msg));
        }

        // 网络ID由客户端提供；如果缺失则拒绝
        let incoming_network_id = node_info.network_id.clone();
        if incoming_network_id.is_empty() {
            let error_msg = "握手请求缺少 network_id".to_string();
            let error_response = Message::error(error_msg.clone());
            peer.read().await.send_message(&error_response).await?;
            {
                let mut peer_guard = peer.write().await;
                peer_guard.update_status(PeerStatus::Error("缺少 network_id".to_string()));
            }
            return Err(anyhow::anyhow!("缺少 network_id"));
        }
        
        // 更新节点信息
        {
            let mut peer_guard = peer.write().await;
            peer_guard.id = node_info.id;
            peer_guard.node_info = Some(node_info.clone());
            peer_guard.update_status(PeerStatus::Authenticated);
        }
        
        // 更新peers映射中的键
        {
            let mut peers = self.peers.write().await;
            // 找到旧的键并移除
            let old_key = peers.iter()
                .find(|(_, v)| Arc::ptr_eq(v, &peer))
                .map(|(k, _)| *k);
            
            if let Some(old_key) = old_key {
                peers.remove(&old_key);
            }
            
            peers.insert(node_info.id, peer.clone());
        }
        
        // 发送握手响应：回显客户端的 network_id
        let mut local_info = self.local_node_info.clone();
        local_info.network_id = incoming_network_id;
        let response = Message::handshake_response(local_info, true);
        
        peer.read().await.send_message(&response).await?;

        Ok(())
    }
    
    /// 处理节点发现请求
    pub async fn handle_handshake_response(
        &self,
        peer: Arc<RwLock<Peer>>, 
        message: &Message,
    ) -> Result<()> {
        let response = HandshakeProtocol::validate_handshake_response(message)
            .map_err(|e| anyhow::anyhow!("握手响应验证失败: {}", e))?;
        
        // 打印更详细的握手响应信息
        let remote_network_id_dbg = response.node_info.metadata.get("network_id").cloned();
        let peer_addr = peer.read().await.addr();
        info!(
            "收到握手响应: 对端地址={}、节点名={}、节点ID={}、网络ID={:?}",
            peer_addr, response.node_info.name, response.node_info.id, remote_network_id_dbg
        );

        if response.success {
            // 网络ID校验（可选）：仅当本地设置了 network_id 时才校验
            let expected_network_id = self.local_node_info.metadata.get("network_id").cloned();
            let remote_network_id = response.node_info.metadata.get("network_id").cloned();
            if expected_network_id.is_some() && expected_network_id != remote_network_id {
                let error_msg = format!(
                    "网络ID不匹配: 本地={:?}, 对端={:?}",
                    expected_network_id, remote_network_id
                );
                warn!("{}", error_msg);
                peer.write().await.update_status(PeerStatus::Error("网络ID不匹配".to_string()));
                return Err(anyhow::anyhow!("网络ID不匹配"));
            }

            let mut peer_guard = peer.write().await;
            peer_guard.id = response.node_info.id;
            peer_guard.node_info = Some(response.node_info.clone());
            peer_guard.update_status(PeerStatus::Authenticated);
            
            info!(
                "握手响应成功: 节点名={}、节点ID={}、网络ID={:?}",
                peer_guard.node_info.as_ref().map(|n| n.name.clone()).unwrap_or_default(),
                peer_guard.id,
                remote_network_id_dbg
            );
        } else {
            let error_msg = response.error_message.unwrap_or_else(|| "握手失败".to_string());
            peer.write().await.update_status(PeerStatus::Error(error_msg.clone()));
            return Err(anyhow::anyhow!("握手失败: {}", error_msg));
        }
        
        Ok(())
    }
    
    /// 处理心跳
    pub async fn handle_ping(&self, peer: Arc<RwLock<Peer>>, _message: &Message) -> Result<()> {
        // 更新最后ping时间
        peer.write().await.update_ping();
        
        // 发送pong响应
        let pong = Message::pong();
        peer.read().await.send_message(&pong).await?;
        
        Ok(())
    }
    
    /// 处理心跳响应
    pub async fn handle_pong(&self, peer: Arc<RwLock<Peer>>, _message: &Message) -> Result<()> {
        peer.write().await.update_ping();
        Ok(())
    }
    
    /// 获取对等节点信息列表
    pub async fn get_peer_info_list(&self) -> Vec<PeerInfo> {
        let peers = self.get_authenticated_peers().await;
        let mut peer_infos = Vec::new();
        
        for peer in peers {
            let peer_guard = peer.read().await;
            if let Some(node_info) = &peer_guard.node_info {
                let peer_info = PeerInfo::new(
                    node_info.id,
                    node_info.listen_addr,
                    node_info.capabilities.clone(),
                );
                peer_infos.push(peer_info);
            }
        }
        
        peer_infos
    }
    
    /// 清理断开的连接
    pub async fn cleanup_disconnected_peers(&self) {
        let mut to_remove = Vec::new();
        
        {
            let peers = self.peers.read().await;
            for (id, peer) in peers.iter() {
                let peer_guard = peer.read().await;
                if !peer_guard.is_connected() {
                    to_remove.push(*id);
                }
            }
        }
        
        for id in to_remove {
            self.remove_peer(&id).await;
        }
    }
    
    /// 获取连接统计信息
    pub async fn get_stats(&self) -> PeerStats {
        let peers = self.peers.read().await;
        let total = peers.len();
        let mut authenticated = 0;
        let mut connecting = 0;
        
        for peer in peers.values() {
            let peer_guard = peer.read().await;
            match peer_guard.status {
                PeerStatus::Authenticated => authenticated += 1,
                PeerStatus::Connecting | PeerStatus::Handshaking => connecting += 1,
                _ => {}
            }
        }
        
        PeerStats {
            total_peers: total,
            authenticated_peers: authenticated,
            connecting_peers: connecting,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PeerStats {
    pub total_peers: usize,
    pub authenticated_peers: usize,
    pub connecting_peers: usize,
}