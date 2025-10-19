use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;
use tokio::time::interval;
use tokio::select;
use anyhow::{Result, Context};
use log::{info, warn, error, debug};
use uuid::Uuid;

use crate::config::Config;
use crate::network::NetworkManager;
use crate::peer::{PeerManager, Peer, PeerStatus};
use crate::protocol::{NodeInfo, Message, MessageType, PeerInfo, HandshakeProtocol};
use crate::router::{MessageRouter, RoutedMessage};

pub struct P2PServer {
    config: Config,
    network_manager: NetworkManager,
    peer_manager: Arc<PeerManager>,
    local_node_info: NodeInfo,
    message_router: Arc<MessageRouter>,
    shutdown_tx: Option<tokio::sync::broadcast::Sender<()>>,
    /// 去抖后的节点列表广播任务句柄
    broadcast_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// 在去抖窗口内需要排除的节点ID（只排除最后一次加入的节点）
    broadcast_exclude_id: Arc<Mutex<Option<Uuid>>>,
}

impl P2PServer {
    pub async fn new(config: Config) -> Result<Self> {
        let network_manager = NetworkManager::new(config.listen_address).await
            .context("创建网络管理器失败")?;
        
        let local_addr = network_manager.local_addr();
        let mut local_node_info = NodeInfo::new(
            format!("p2p_node_{}", local_addr.port()),
            local_addr,
            config.network_id.clone(), // 传递 network_id
        );
        local_node_info.network_id = config.network_id.clone();
        
        let peer_manager = Arc::new(PeerManager::new(
            local_node_info.clone(),
            config.max_connections,
        ));
        let message_router = Arc::new(MessageRouter::new(
            local_node_info.id,
            peer_manager.clone(),
        ));
        // 启动路由器的消息缓存清理任务
        let _cache_task = message_router.start_cache_cleanup_task();
        
        info!("P2P服务器初始化完成");
        info!("节点ID: {}", local_node_info.id);
        info!("监听地址: {}", local_addr);
        info!("最大连接数: {}", config.max_connections);
        
        Ok(Self {
            config,
            network_manager,
            peer_manager,
            local_node_info,
            message_router,
            shutdown_tx: None,
            broadcast_task: Arc::new(Mutex::new(None)),
            broadcast_exclude_id: Arc::new(Mutex::new(None)),
        })
    }

    /// 调度一次去抖的节点列表广播，将在窗口结束后向所有节点推送当前列表
    async fn schedule_peerlist_broadcast(&self, exclude_id: Option<Uuid>) {
        // 记录最后一次加入的节点ID，用于在广播时排除该节点
        *self.broadcast_exclude_id.lock().await = exclude_id;

        // 取消已有任务并重置窗口
        if let Some(handle) = self.broadcast_task.lock().await.take() {
            handle.abort();
        }

        let peer_manager = self.peer_manager.clone();
        let exclude_arc = self.broadcast_exclude_id.clone();
        let delay_ms = self.config.peerlist_broadcast_debounce_ms;

        let handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            // 取出并清空待排除ID
            let exclude_id = {
                let mut ex = exclude_arc.lock().await;
                std::mem::take(&mut *ex)
            };

            // 广播（按接收者定制，不发送给处于排除列表的节点）
            let peers = peer_manager.get_authenticated_peers().await;
            for p in peers {
                let pid = p.read().await.id;
                if exclude_id == Some(pid) { continue; }
                let infos = peer_manager.get_peer_info_list_excluding(Some(pid)).await;
                let msg = Message::discovery_response(infos);
                if let Err(e) = p.read().await.send_message(&msg).await {
                    warn!("去抖广播节点列表到 {} 失败: {}", p.read().await.addr(), e);
                }
            }
        });

        *self.broadcast_task.lock().await = Some(handle);
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
        let (hb_res, cl_res, st_res) = tokio::join!(heartbeat_task, cleanup_task, stats_task);
        if let Err(e) = hb_res {
            warn!("心跳任务结束时发生错误: {}", e);
        }
        if let Err(e) = cl_res {
            warn!("清理任务结束时发生错误: {}", e);
        }
        if let Err(e) = st_res {
            warn!("统计任务结束时发生错误: {}", e);
        }
        
        info!("P2P服务器已停止");
        Ok(())
    }
    
    async fn handle_udp_packet(&self, data: Vec<u8>, sender_addr: std::net::SocketAddr) -> Result<()> {
        // 打印最原始的UDP数据包内容
        if let Ok(text) = std::str::from_utf8(&data) {
            info!("收到来自 {} 的原始UDP数据包: {}", sender_addr, text);
        } else {
            info!("收到来自 {} 的原始UDP数据包 (非UTF-8): {:?}", sender_addr, data);
        }
        
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
                info!(
                    "已发送ACK: ack_for={} 给 {} (seq={:?})",
                    message.id,
                    sender_addr,
                    message.sequence_number
                );
            }
        }
        
        match message.message_type {
            MessageType::HandshakeRequest => {
                info!("处理握手请求消息，来自 {}", peer.read().await.addr());
                // 先解析以便在路由表中添加直连路由
                if let Ok(node_info) = HandshakeProtocol::validate_handshake_request(message) {
                    self.message_router
                        .update_routing_table(node_info.id, node_info.id, 1)
                        .await;
                    // 处理握手
                    self.peer_manager.handle_handshake_request(peer, message).await?;
                    // 去抖调度一次广播，排除该新加入节点，避免重复推送
                    self.schedule_peerlist_broadcast(Some(node_info.id)).await;
                    return Ok(());
                }
                // 验证失败仍尝试交由处理函数返回错误
                self.peer_manager.handle_handshake_request(peer, message).await?;
            }
            MessageType::HandshakeResponse => {
                info!("处理握手响应消息，来自 {}", peer.read().await.addr());
                self.peer_manager.handle_handshake_response(peer.clone(), message).await?;
                // 握手成功后，添加直连路由（距离为1）
                let remote_id = peer.read().await.id;
                self.message_router
                    .update_routing_table(remote_id, remote_id, 1)
                    .await;
            }
            MessageType::Ping => {
                info!("收到Ping，来自 {}", peer.read().await.addr());
                self.peer_manager.handle_ping(peer, message).await?;
            }
            MessageType::Pong => {
                info!("收到Pong，来自 {}", peer.read().await.addr());
                self.peer_manager.handle_pong(peer, message).await?;
            }
            MessageType::DiscoveryRequest => {
                Self::handle_discovery_request(&self.peer_manager, peer, message).await?;
            }
            MessageType::DiscoveryResponse => {
                info!("收到节点发现响应，来自 {}", peer.read().await.addr());
                // 解析对端提供的节点信息列表，并更新路由表（经该对端的下一跳，距离为2）
                if let Ok(peer_list) = serde_json::from_value::<Vec<PeerInfo>>(message.payload.clone()) {
                    let next_hop = peer.read().await.id;
                    for p in &peer_list {
                        // 跳过本地节点和对端自身
                        if p.id == self.local_node_info.id || p.id == next_hop {
                            continue;
                        }
                        self.message_router
                            .update_routing_table(p.id, next_hop, 2)
                            .await;
                    }
                    debug!("从 {} 更新路由项 {} 条", peer.read().await.addr(), peer_list.len());
                } else {
                    warn!("解析节点发现响应失败");
                }
            }
            MessageType::P2PConnect => {
                info!("处理 P2P 直连协调请求，来自 {}", peer.read().await.addr());
                let target_id = message
                    .payload
                    .get("peer_id")
                    .and_then(|v| v.as_str())
                    .and_then(|s| uuid::Uuid::parse_str(s).ok());

                if let Some(target_id) = target_id {
                    let requester_id = peer.read().await.id;
                    if requester_id == target_id {
                        let err = Message::error("不能与自身建立直连".to_string());
                        peer.read().await.send_message(&err).await?;
                    } else if let Some(target_peer) = self.peer_manager.get_peer(&target_id).await {
                        if !target_peer.read().await.is_authenticated() {
                            let err = Message::error(format!("目标节点未认证: {}", target_id));
                            peer.read().await.send_message(&err).await?;
                        } else {
                            let requester_addr = peer.read().await.addr();
                            let target_addr = target_peer.read().await.addr();

                            // 通知请求方目标的直连信息
                            let msg_to_requester = Message::new(
                                MessageType::P2PConnect,
                                serde_json::json!({
                                    "peer_id": target_id.to_string(),
                                    "peer_addr": target_addr.to_string()
                                }),
                            );
                            peer.read().await.send_message(&msg_to_requester).await?;

                            // 通知目标方请求方的直连信息
                            let msg_to_target = Message::new(
                                MessageType::P2PConnect,
                                serde_json::json!({
                                    "peer_id": requester_id.to_string(),
                                    "peer_addr": requester_addr.to_string()
                                }),
                            );
                            target_peer.read().await.send_message(&msg_to_target).await?;

                            debug!(
                                "P2P 直连协调成功: requester={}({}), target={}({})",
                                requester_id,
                                requester_addr,
                                target_id,
                                target_addr
                            );
                        }
                    } else {
                        let err = Message::error(format!("目标节点未找到或不可达: {}", target_id));
                        peer.read().await.send_message(&err).await?;
                    }
                } else {
                    let err = Message::error("缺少或无效的 peer_id".to_string());
                    peer.read().await.send_message(&err).await?;
                }
            }
            MessageType::Data => {
                info!("收到数据消息，来自 {}", peer.read().await.addr());
                // 尝试作为路由消息处理
                match RoutedMessage::from_message(message) {
                    Ok(routed) => {
                        self.message_router.forward_message(routed).await?;
                    }
                    Err(_) => {
                        // 非路由包，按原有逻辑处理
                        self.handle_data_message(peer, message).await?;
                    }
                }
            }
            MessageType::Disconnect => {
                info!("节点 {} 请求断开连接", peer.read().await.id);
                peer.write().await.update_status(PeerStatus::Disconnected);
                // 移除相关路由
                let pid = peer.read().await.id;
                self.message_router.remove_node_routes(&pid).await;
                // 立即从PeerManager移除，并调度一次去抖广播以通知其他节点
                self.peer_manager.remove_peer(&pid).await;
                // 断开不需要排除某个接收者
                self.schedule_peerlist_broadcast(None).await;
            }
            MessageType::Ack => {
                info!("收到ACK消息: ack_for={:?} 来自 {}", message.ack_for, peer.read().await.addr());
                // 处理ACK逻辑（如果需要）
            }
            MessageType::ListNodesRequest => {
                info!("处理列出节点请求消息，来自 {}", peer.read().await.addr());
                let all_peers = self.peer_manager.get_all_peers().await;
                let mut all_peers_info = Vec::new();
                for p in all_peers {
                    let p_read = p.read().await;
                    if let Some(mut node_info) = p_read.node_info.clone() {
                        node_info.listen_addr = p_read.addr();
                        all_peers_info.push(node_info);
                    }
                }
                let response = Message::list_nodes_response(all_peers_info);
                peer.read().await.send_message(&response).await?;
            }
            MessageType::Error => {
                warn!("收到错误消息: {:?} 来自 {}", message.payload, peer.read().await.addr());
            }
            _ => {
                warn!("未知消息类型: {:?}", message.message_type);
            }
        }
        
        Ok(())
    }

    #[allow(dead_code)]
    async fn handle_peer_messages(
        &self,
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
                MessageType::DiscoveryResponse => {
                    // 更新路由表（经该对端的下一跳，距离为2）
                    if let Ok(peer_list) = serde_json::from_value::<Vec<PeerInfo>>(message.payload.clone()) {
                        let next_hop = peer.read().await.id;
                        for p in peer_list {
                            if p.id == next_hop { continue; }
                        }
                    }
                    Ok(())
                }
                MessageType::Data => {
                    // 尝试作为路由消息处理
                    match RoutedMessage::from_message(&message) {
                        Ok(routed) => {
                            // 这里无法访问 server 的 router；该函数目前未在运行循环中使用
                            debug!("收到路由数据消息，route_id={:?}", routed.route_id);
                            Ok(())
                        }
                        Err(_) => {
                            self.handle_data_message(peer.clone(), &message).await
                        }
                    }
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
        let requester_id = peer.read().await.id;
        let peer_infos = peer_manager.get_peer_info_list_excluding(Some(requester_id)).await;
        let response = Message::discovery_response(peer_infos);
        
        peer.read().await.send_message(&response).await?;
        
        debug!("发送节点发现响应给 {}", peer.read().await.addr());
        
        Ok(())
    }
    
    async fn handle_data_message(
        &self,
        peer: Arc<tokio::sync::RwLock<Peer>>,
        message: &Message,
    ) -> Result<()> {
        // 这里可以实现数据消息的处理逻辑
        // 例如：转发给其他节点、存储数据等
        
        debug!("从 {} 接收到数据消息: {:?}", peer.read().await.addr(), message.payload);
        
        // 命令：获取路由快照
        if let Some(obj) = message.payload.as_object() {
            if let Some(cmd) = obj.get("cmd").and_then(|v| v.as_str()) {
                if cmd == "get_routes" {
                    let snapshot = self.message_router.get_routing_table_snapshot().await;
                    let routes: Vec<serde_json::Value> = snapshot
                        .into_iter()
                        .map(|(dest, next_hop, distance)| serde_json::json!({
                            "destination": dest,
                            "next_hop": next_hop,
                            "distance": distance
                        }))
                        .collect();
                    let resp = Message::data(serde_json::json!({ "routes": routes }));
                    peer.read().await.send_message(&resp).await?;
                    return Ok(());
                }
            }
        }

        // 简单的回显响应（默认行为）
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

    /// 通过路由向指定节点发送数据
    #[allow(dead_code)]
    pub async fn send_routed_data(
        &self,
        destination: Uuid,
        data: serde_json::Value,
        max_hops: u32,
    ) -> Result<()> {
        let message = Message::data(data);
        self.message_router.route_message(message, destination, max_hops).await
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ServerStats {
    pub node_id: Uuid,
    pub listen_address: std::net::SocketAddr,
    pub peer_stats: crate::peer::PeerStats,
    pub uptime: u64,
}