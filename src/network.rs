use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use anyhow::{Result, Context};
use log::{info, debug};


use crate::protocol::Message;

/// UDP连接抽象
#[derive(Debug, Clone)]
pub struct Connection {
    socket: Arc<UdpSocket>,
    peer_addr: SocketAddr,

    #[allow(dead_code)]
    local_addr: SocketAddr,
}

impl Connection {
    pub fn new(socket: Arc<UdpSocket>, peer_addr: SocketAddr, local_addr: SocketAddr) -> Self {
        Self { 
            socket, 
            peer_addr,
            local_addr,
        }
    }
    
    pub fn peer_addr(&self) -> SocketAddr {
        self.peer_addr
    }
    
    #[allow(dead_code)]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    
    /// 发送消息
    pub async fn send_message(&self, message: &Message) -> Result<()> {
        let data = serde_json::to_vec(message)
            .context("序列化消息失败")?;
        
        // UDP直接发送数据，不需要长度前缀
        let bytes_sent = self.socket.send_to(&data, self.peer_addr).await
            .context("发送UDP消息失败")?;
        
        debug!("发送UDP消息到 {}: {} bytes", self.peer_addr, bytes_sent);
        Ok(())
    }
    
    /// 接收消息（注意：UDP是无连接的，这个方法主要用于兼容性）
    pub async fn receive_message(&self) -> Result<Option<Message>> {
        // 对于UDP，接收消息的逻辑会在NetworkManager中处理
        // 这里返回None表示没有消息（UDP是无连接的）
        Ok(None)
    }
}

/// 网络管理器
pub struct NetworkManager {
    socket: Arc<UdpSocket>,
    local_addr: SocketAddr,
    // 存储已知的对等节点连接
    connections: Arc<RwLock<HashMap<SocketAddr, Arc<Connection>>>>,
}

impl NetworkManager {
    /// 创建新的网络管理器
    pub async fn new(bind_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await
            .context(format!("绑定UDP地址 {} 失败", bind_addr))?;
        
        let local_addr = socket.local_addr()
            .context("获取本地地址失败")?;
        
        info!("UDP网络管理器已绑定到 {}", local_addr);
        
        Ok(Self {
            socket: Arc::new(socket),
            local_addr,
            connections: Arc::new(RwLock::new(HashMap::new())),
        })
    }
    
    /// 获取本地监听地址
    #[allow(dead_code)]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
    
    /// 接收UDP数据包和发送者地址
    pub async fn receive_from(&self) -> Result<(Vec<u8>, SocketAddr)> {
        let mut buffer = vec![0u8; 65536]; // UDP最大包大小
        let (len, peer_addr) = self.socket.recv_from(&mut buffer).await
            .context("接收UDP数据失败")?;
        
        buffer.truncate(len);
        debug!("从 {} 接收UDP数据: {} bytes", peer_addr, len);
        
        Ok((buffer, peer_addr))
    }
    
    /// 解析接收到的数据为消息
    pub fn parse_message(&self, data: &[u8]) -> Result<Message> {
        let message: Message = serde_json::from_slice(data)
            .context("反序列化UDP消息失败")?;
        Ok(message)
    }
    
    /// 获取或创建到指定地址的连接
    pub async fn get_or_create_connection(&self, peer_addr: SocketAddr) -> Arc<Connection> {
        let mut connections = self.connections.write().await;
        
        if let Some(connection) = connections.get(&peer_addr) {
            connection.clone()
        } else {
            let connection = Arc::new(Connection::new(
                self.socket.clone(),
                peer_addr,
                self.local_addr,
            ));
            connections.insert(peer_addr, connection.clone());
            info!("创建到 {} 的新UDP连接", peer_addr);
            connection
        }
    }
    
    /// 移除连接
    #[allow(dead_code)]
    pub async fn remove_connection(&self, peer_addr: &SocketAddr) {
        let mut connections = self.connections.write().await;
        if connections.remove(peer_addr).is_some() {
            info!("移除到 {} 的UDP连接", peer_addr);
        }
    }
    
    /// 获取所有活跃连接
    #[allow(dead_code)]
    pub async fn get_all_connections(&self) -> Vec<Arc<Connection>> {
        let connections = self.connections.read().await;
        connections.values().cloned().collect()
    }
    
    /// 主动连接到对等节点（UDP中实际上是创建连接对象）
    #[allow(dead_code)]
    pub async fn connect_to_peer(&self, addr: SocketAddr) -> Result<Arc<Connection>> {
        let connection = self.get_or_create_connection(addr).await;
        info!("准备连接到UDP对等节点 {}", addr);
        Ok(connection)
    }
    
    /// 发送消息到指定地址
    pub async fn send_to(&self, message: &Message, addr: SocketAddr) -> Result<()> {
        let data = serde_json::to_vec(message)
            .context("序列化消息失败")?;
        
        let bytes_sent = self.socket.send_to(&data, addr).await
            .context("发送UDP消息失败")?;
        
        debug!("直接发送UDP消息到 {}: {} bytes", addr, bytes_sent);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_network_manager_creation() {
        let addr = "127.0.0.1:0".parse().unwrap();
        let manager = NetworkManager::new(addr).await.unwrap();
        assert!(manager.local_addr().port() > 0);
    }
}