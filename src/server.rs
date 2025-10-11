use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tokio::select;
use anyhow::{Result, Context};
use log::{info, warn, error, debug};
use uuid::Uuid;

use crate::config::Config;
use crate::network::NetworkManager;
use crate::peer::{PeerManager, Peer, PeerStatus};
use crate::protocol::{NodeInfo, Message, MessageType};

pub struct P2PServer {
    config: Config,
    network_manager: NetworkManager,
    peer_manager: Arc<PeerManager>,
    local_node_info: NodeInfo,
    shutdown_tx: Option<tokio::sync::broadcast::Sender<()>>,
}

impl P2PServer {
    pub async fn new(config: Config) -> Result<Self> {
        let network_manager = NetworkManager::new(config.listen_address).await
            .context("创建网络管理器失败")?;
        
        let local_addr = network_manager.local_addr();
        let mut local_node_info = NodeInfo::new(
            format!("p2p_node_{}", local_addr.port()),
            local_addr,
        );
        
        // 添加服务器特定的元数据
        local_node_info.add_metadata("server_type".to_string(), "handshake_server".to_string());
        local_node_info.add_metadata("max_connections".to_string(), config.max_connections.to_string());
        
        let peer_manager = Arc::new(PeerManager::new(
            local_node_info.clone(),
            config.max_connections,
        ));
        
        info!("P2P服务器初始化完成");
        info!("节点ID: {}", local_node_info.id);
        info!("监听地址: {}", local_addr);
        info!("最大连接数: {}", config.max_connections);
        
        Ok(Self {
            config,
            network_manager,
            peer_manager,
            local_node_info,
            shutdown_tx: None,
        })
    }
    
    pub async fn run(&mut self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::broadcast::channel(1);
        self.shutdown_tx = Some(shutdown_tx);
        
        info!("P2P服务器开始运行...");
        
        // 启动心跳任务
        let heartbeat_task = self.start_heartbeat_task();
        
        // 启动清理任务
        let cleanup_task = self.start_cleanup_task();
        
        // 启动统计任务
        let stats_task = self.start_stats_task();
        
        // 主循环：接收UDP数据包
        loop {
            select! {
                // 接收UDP数据包
                packet_result = self.network_manager.receive_from() => {
                    match packet_result {
                        Ok((data, sender_addr)) => {
                            if let Err(e) = self.handle_udp_packet(data, sender_addr).await {
                                error!("处理UDP数据包失败: {}", e);
                            }
                        }
                        Err(e) => {
                            error!("接收UDP数据包失败: {}", e);
                        }
                    }
                }
                
                // 监听关闭信号
                _ = shutdown_rx.recv() => {
                    info!("收到关闭信号，正在停止服务器...");
                    break;
                }
            }
        }
        
        // 等待所有任务完成
        tokio::join!(heartbeat_task, cleanup_task, stats_task);
        
        info!("P2P服务器已停止");
        Ok(())
    }
    
    async fn handle_udp_packet(&self, data: Vec<u8>, sender_addr: std::net::SocketAddr) -> Result<()> {
        debug!("处理来自 {} 的UDP数据包: {} bytes", sender_addr, data.len());
        
        // 解析消息
        let mut message = self.network_manager.parse_message(&data)?;
        message.sender_addr = Some(sender_addr);
        
        // 获取或创建连接
        let connection = self.network_manager.get_or_create_connection(sender_addr).await;
        
        // 获取或创建peer
        let peer = self.peer_manager.get_or_create_peer_by_addr(connection).await?;
        
        // 处理消息
        self.handle_message(peer, &message).await?;
        
        Ok(())
    }
    
    async fn handle_message(
        &self,
        peer: Arc<tokio::sync::RwLock<Peer>>,
        message: &Message,
    ) -> Result<()> {
        debug!("处理消息类型: {:?} 来自 {}", message.message_type, message.sender_addr.unwrap_or_else(|| "0.0.0.0:0".parse().unwrap()));
        
        // 如果需要确认，发送ACK
        if message.requires_ack {
            let ack_message = Message::ack(message.id, self.local_node_info.listen_addr);
            if let Some(sender_addr) = message.sender_addr {
                if let Err(e) = self.network_manager.send_to(&ack_message, sender_addr).await {
                    warn!("发送ACK失败: {}", e);
                }
            }
        }
        
        match message.message_type {
            MessageType::HandshakeRequest => {
                self.peer_manager.handle_handshake_request(peer, message).await?;
            }
            MessageType::HandshakeResponse => {
                self.peer_manager.handle_handshake_response(peer, message).await?;
            }
            MessageType::Ping => {
                self.peer_manager.handle_ping(peer, message).await?;
            }
            MessageType::Pong => {
                self.peer_manager.handle_pong(peer, message).await?;
            }
            MessageType::DiscoveryRequest => {
                Self::handle_discovery_request(&self.peer_manager, peer, message).await?;
            }
            MessageType::Data => {
                Self::handle_data_message(peer, message).await?;
            }
            MessageType::Disconnect => {
                info!("节点 {} 请求断开连接", peer.read().await.id);
                peer.write().await.update_status(PeerStatus::Disconnected);
            }
            MessageType::Ack => {
                debug!("收到ACK消息: {:?}", message.ack_for);
                // 处理ACK逻辑（如果需要）
            }
            MessageType::Error => {
                warn!("收到错误消息: {:?}", message.payload);
            }
            _ => {
                warn!("未知消息类型: {:?}", message.message_type);
            }
        }
        
        Ok(())
    }

    async fn handle_peer_messages(
        peer_manager: Arc<PeerManager>,
        peer: Arc<tokio::sync::RwLock<Peer>>,
    ) -> Result<()> {
        let peer_id = peer.read().await.id;
        let peer_addr = peer.read().await.addr();
        
        debug!("开始处理来自 {} 的消息", peer_addr);
        
        loop {
            let message = match peer.read().await.receive_message().await {
                Ok(Some(msg)) => msg,
                Ok(None) => {
                    info!("对等节点 {} 断开连接", peer_addr);
                    break;
                }
                Err(e) => {
                    warn!("从对等节点 {} 接收消息失败: {}", peer_addr, e);
                    break;
                }
            };
            
            debug!("从 {} 接收到消息: {:?}", peer_addr, message.message_type);
            
            let result = match message.message_type {
                MessageType::HandshakeRequest => {
                    peer_manager.handle_handshake_request(peer.clone(), &message).await
                }
                MessageType::HandshakeResponse => {
                    peer_manager.handle_handshake_response(peer.clone(), &message).await
                }
                MessageType::Ping => {
                    peer_manager.handle_ping(peer.clone(), &message).await
                }
                MessageType::Pong => {
                    peer_manager.handle_pong(peer.clone(), &message).await
                }
                MessageType::DiscoveryRequest => {
                    Self::handle_discovery_request(&peer_manager, peer.clone(), &message).await
                }
                MessageType::Data => {
                    Self::handle_data_message(peer.clone(), &message).await
                }
                MessageType::Disconnect => {
                    info!("对等节点 {} 请求断开连接", peer_addr);
                    break;
                }
                MessageType::Error => {
                    warn!("从对等节点 {} 接收到错误消息: {:?}", peer_addr, message.payload);
                    Ok(())
                }
                _ => {
                    warn!("未知消息类型: {:?}", message.message_type);
                    Ok(())
                }
            };
            
            if let Err(e) = result {
                error!("处理消息失败: {}", e);
                
                // 发送错误响应
                let error_msg = Message::error(format!("处理消息失败: {}", e));
                if let Err(send_err) = peer.read().await.send_message(&error_msg).await {
                    error!("发送错误消息失败: {}", send_err);
                }
                
                // 对于严重错误，断开连接
                peer.write().await.update_status(PeerStatus::Error(e.to_string()));
                break;
            }
        }
        
        // 清理断开的对等节点
        peer_manager.remove_peer(&peer_id).await;
        
        Ok(())
    }
    
    async fn handle_discovery_request(
        peer_manager: &Arc<PeerManager>,
        peer: Arc<tokio::sync::RwLock<Peer>>,
        _message: &Message,
    ) -> Result<()> {
        let peer_infos = peer_manager.get_peer_info_list().await;
        let response = Message::discovery_response(peer_infos);
        
        peer.read().await.send_message(&response).await?;
        
        debug!("发送节点发现响应给 {}", peer.read().await.addr());
        
        Ok(())
    }
    
    async fn handle_data_message(
        peer: Arc<tokio::sync::RwLock<Peer>>,
        message: &Message,
    ) -> Result<()> {
        // 这里可以实现数据消息的处理逻辑
        // 例如：转发给其他节点、存储数据等
        
        debug!("从 {} 接收到数据消息: {:?}", peer.read().await.addr(), message.payload);
        
        // 简单的回显响应
        let echo_response = Message::data(serde_json::json!({
            "echo": message.payload,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        }));
        
        peer.read().await.send_message(&echo_response).await?;
        
        Ok(())
    }
    
    fn start_heartbeat_task(&self) -> tokio::task::JoinHandle<()> {
        let peer_manager = self.peer_manager.clone();
        let heartbeat_interval = self.config.heartbeat_interval;
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(heartbeat_interval));
            
            loop {
                interval.tick().await;
                
                let peers = peer_manager.get_authenticated_peers().await;
                let peer_count = peers.len();
                
                for peer in peers {
                    let ping_message = Message::ping();
                    if let Err(e) = peer.read().await.send_message(&ping_message).await {
                        warn!("发送心跳失败: {}", e);
                        peer.write().await.update_status(PeerStatus::Error(e.to_string()));
                    }
                }
                
                debug!("发送心跳给 {} 个节点", peer_count);
            }
        })
    }
    
    fn start_cleanup_task(&self) -> tokio::task::JoinHandle<()> {
        let peer_manager = self.peer_manager.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(60)); // 每分钟清理一次
            
            loop {
                interval.tick().await;
                peer_manager.cleanup_disconnected_peers().await;
                debug!("执行对等节点清理任务");
            }
        })
    }
    
    fn start_stats_task(&self) -> tokio::task::JoinHandle<()> {
        let peer_manager = self.peer_manager.clone();
        
        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(300)); // 每5分钟输出一次统计
            
            loop {
                interval.tick().await;
                
                let stats = peer_manager.get_stats().await;
                info!(
                    "节点统计 - 总数: {}, 已认证: {}, 连接中: {}",
                    stats.total_peers,
                    stats.authenticated_peers,
                    stats.connecting_peers
                );
            }
        })
    }
    
    /// 主动连接到其他节点
    pub async fn connect_to_peer(&self, addr: std::net::SocketAddr) -> Result<()> {
        info!("尝试连接到UDP对等节点: {}", addr);
        
        // 发送握手请求
        let handshake_request = Message::new_with_ack(
            MessageType::HandshakeRequest,
            serde_json::to_value(&self.local_node_info)?,
            self.local_node_info.listen_addr,
            0, // 序列号
        );
        
        self.network_manager.send_to(&handshake_request, addr).await?;
        
        info!("已向 {} 发送握手请求", addr);
        Ok(())
    }
    
    /// 获取服务器统计信息
    pub async fn get_stats(&self) -> ServerStats {
        let peer_stats = self.peer_manager.get_stats().await;
        
        ServerStats {
            node_id: self.local_node_info.id,
            listen_address: self.config.listen_address,
            peer_stats,
            uptime: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    /// 优雅关闭服务器
    pub async fn shutdown(&self) -> Result<()> {
        if let Some(tx) = &self.shutdown_tx {
            tx.send(()).context("发送关闭信号失败")?;
        }
        
        // 向所有连接的节点发送断开消息
        let peers = self.peer_manager.get_all_peers().await;
        for peer in peers {
            let disconnect_msg = Message::disconnect("服务器关闭".to_string());
            if let Err(e) = peer.read().await.send_message(&disconnect_msg).await {
                warn!("发送断开消息失败: {}", e);
            }
        }
        
        info!("服务器关闭完成");
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ServerStats {
    pub node_id: Uuid,
    pub listen_address: std::net::SocketAddr,
    pub peer_stats: crate::peer::PeerStats,
    pub uptime: u64,
}