use anyhow::Result;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration, interval};
use std::net::SocketAddr;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use p2p_handshake_server::protocol::{Message, MessageType, HandshakeResponse, NodeInfo, ListNodesResponse, PeerInfo};
use p2p_handshake_server::router::RoutedMessage;

// 服务器状态枚举
#[derive(Debug, Clone, PartialEq)]
enum ServerStatus {
    Online,
    Offline,
    Unknown,
}

// 客户端状态结构
struct ClientState {
    server_status: ServerStatus,
    last_server_heartbeat: Option<std::time::Instant>,
    heartbeat_timeout_count: u32,
}

impl ClientState {
    fn new() -> Self {
        Self {
            server_status: ServerStatus::Unknown,
            last_server_heartbeat: None,
            heartbeat_timeout_count: 0,
        }
    }

    fn mark_server_online(&mut self) {
        self.server_status = ServerStatus::Online;
        self.last_server_heartbeat = Some(std::time::Instant::now());
        self.heartbeat_timeout_count = 0;
    }

    fn check_server_timeout(&mut self) -> bool {
        if let Some(last_heartbeat) = self.last_server_heartbeat {
            // 服务器心跳间隔是30秒，我们允许45秒的容忍度
            if last_heartbeat.elapsed().as_secs() > 45 {
                self.heartbeat_timeout_count += 1;
                // 连续2次超时才标记为离线，避免网络抖动误判
                if self.heartbeat_timeout_count >= 2 {
                    self.server_status = ServerStatus::Offline;
                    return true;
                }
            }
        } else {
            // 握手后90秒内没有收到心跳，标记为离线
            self.heartbeat_timeout_count += 1;
            if self.heartbeat_timeout_count >= 3 {
                self.server_status = ServerStatus::Offline;
                return true;
            }
        }
        false
    }

    fn is_server_online(&self) -> bool {
        self.server_status == ServerStatus::Online
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // 从命令行参数获取服务器地址和客户端名称
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        println!("用法: simple_client <服务器地址:端口> <客户端名称>");
        return Ok(());
    }
    let server_addr: SocketAddr = args[1].parse()?;
    let client_name = &args[2];

    println!("将连接到服务器: {}", server_addr);

    // 创建客户端
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    let local_addr = socket.local_addr()?;
    println!("客户端 [{}] 地址: {}", client_name, local_addr);

    // 准备节点信息（network_id 固定为 test）
    let node_info = NodeInfo::new(client_name.to_string(), local_addr, "test".to_string());

    // 进行握手
    let own_node_id = match handshake(&socket, server_addr, node_info.clone()).await {
        Ok(id) => id,
        Err(e) => {
            println!("握手失败: {}", e);
            return Ok(());
        }
    };
    println!("握手成功! 你的 Node ID 是: {}", own_node_id);
    println!("公网地址可能为: (请查看服务器日志确认)");

    // 用于存储对等节点信息
    let peers = Arc::new(Mutex::new(HashMap::<Uuid, PeerInfo>::new()));
    let p2p_peers = Arc::new(Mutex::new(HashMap::<Uuid, SocketAddr>::new()));
    
    // 客户端状态管理
    let client_state = Arc::new(Mutex::new(ClientState::new()));
    client_state.lock().await.mark_server_online(); // 握手成功说明服务器在线

    // 启动服务器状态监控任务（被动监听心跳）
    let monitor_state = client_state.clone();
    tokio::spawn(async move {
        let mut check_interval = interval(Duration::from_secs(20)); // 每20秒检查一次服务器状态
        loop {
            check_interval.tick().await;
            
            let mut state = monitor_state.lock().await;
            let was_online = state.is_server_online();
            let became_offline = state.check_server_timeout();
            
            if became_offline && was_online {
                println!("服务器状态: 离线 - 切换到P2P模式 (心跳超时)");
            } else if !was_online && state.is_server_online() {
                println!("服务器状态: 在线");
            }
        }
    });

    // 启动消息接收任务
    let recv_socket = socket.clone();
    let peers_clone = peers.clone();
    let p2p_peers_clone = p2p_peers.clone();
    let own_id_clone = own_node_id;
    let state_clone = client_state.clone();
    tokio::spawn(async move {
        loop {
            if let Some(msg) = receive_message(&recv_socket).await.unwrap_or(None) {
                 match msg.message_type {
                    MessageType::Data => {
                        if let Ok(rm) = RoutedMessage::from_message(&msg) {
                            println!("\n收到消息: [来源: {}] [内容: {}]", rm.source_node, rm.original_message.payload);
                        } else {
                            println!("\n收到非路由数据: {:?}", msg.payload);
                        }
                    },
                    MessageType::DiscoveryResponse => {
                        if let Ok(peers_data) = serde_json::from_value::<Vec<PeerInfo>>(msg.payload) {
                            println!("--- 发现节点列表 (自动下发) ---");
                            let mut peers_map = peers_clone.lock().await;
                            for peer in peers_data {
                                if peer.id == own_id_clone { continue; }
                                println!("- ID: {}, 地址: {}", peer.id, peer.addr);
                                peers_map.insert(peer.id, peer);
                            }
                            println!("---------------------------");
                            
                            // 如果服务器在线，主动建立P2P连接
                            if state_clone.lock().await.is_server_online() {
                                let peers_to_connect: Vec<_> = peers_map.keys().cloned().collect();
                                drop(peers_map); // 释放锁
                                
                                for peer_id in peers_to_connect {
                                    // 发起P2P连接请求
                                    let p2p_req = Message::initiate_p2p(peer_id);
                                    if let Err(e) = send_message(&recv_socket, &p2p_req, server_addr).await {
                                        println!("发送 P2P 连接请求失败: {}", e);
                                    }
                                }
                            }
                        } else {
                            println!("无法解析节点发现响应");
                        }
                    },
                    MessageType::ListNodesResponse => {
                        if let Ok(list_response) = serde_json::from_value::<ListNodesResponse>(msg.payload) {
                            println!("--- 在线节点列表 (手动刷新) ---");
                            let mut peers_map = peers_clone.lock().await;
                            for node in list_response.nodes {
                                if node.id == own_id_clone { continue; }
                                println!("- ID: {}, 名称: {}, 地址: {}", node.id, node.name, node.listen_addr);
                                let peer_info = PeerInfo {
                                    id: node.id,
                                    addr: node.listen_addr,
                                    last_seen: 0,
                                    capabilities: node.capabilities,
                                };
                                peers_map.insert(node.id, peer_info);
                            }
                            println!("---------------------");
                        } else {
                            println!("无法解析节点列表响应");
                        }
                    },
                     MessageType::P2PConnect => {
                        if let (Some(peer_addr), Some(peer_id)) = (
                            msg.payload.get("peer_addr").and_then(|v| v.as_str()).and_then(|s| s.parse::<SocketAddr>().ok()),
                            msg.payload.get("peer_id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok())
                        ) {
                            if peer_id == own_id_clone {
                                // 忽略与自身的直连指令
                            } else {
                                println!("收到 P2P 连接指令，目标地址: {}，目标ID: {}", peer_addr, peer_id);
                                let mut p2p_peers_map = p2p_peers_clone.lock().await;
                                p2p_peers_map.insert(peer_id, peer_addr);

                                // Send a punch-through packet
                                let punch_msg = Message::ping();
                                let recv_socket_clone = recv_socket.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = send_message(&recv_socket_clone, &punch_msg, peer_addr).await {
                                        println!("打洞失败: {}", e);
                                    } else {
                                        println!("P2P连接建立成功: {}", peer_id);
                                    }
                                });
                            }
                        }
                    },
                    MessageType::Ack => {
                         // 可以选择忽略或处理Ack
                    },
                    MessageType::Ping => {
                        // 收到服务器的心跳ping
                        if msg.sender_addr == Some(server_addr) {
                            // 更新服务器心跳时间
                            state_clone.lock().await.mark_server_online();
                            
                            // 回复Pong
                            let pong = Message::new(MessageType::Pong, serde_json::Value::Null);
                            let recv_socket_clone = recv_socket.clone();
                            tokio::spawn(async move {
                                if let Err(e) = send_message(&recv_socket_clone, &pong, server_addr).await {
                                    println!("回复服务器 Pong 失败: {}", e);
                                }
                            });
                        } else {
                            // 收到其他节点的ping，回复pong
                            if let Some(addr) = msg.sender_addr {
                                let pong = Message::new(MessageType::Pong, serde_json::Value::Null);
                                let recv_socket_clone = recv_socket.clone();
                                tokio::spawn(async move {
                                    if let Err(e) = send_message(&recv_socket_clone, &pong, addr).await {
                                        println!("回复节点 Pong 失败: {}", e);
                                    }
                                });
                            }
                        }
                    },
                    MessageType::Pong => {
                        // 收到pong响应（可能是P2P连接确认）
                        if msg.sender_addr != Some(server_addr) {
                            println!("收到节点Pong响应: {:?}", msg.sender_addr);
                        }
                    },
                    MessageType::Error => {
                        if let Some(err) = msg.payload.get("error").and_then(|v| v.as_str()) {
                            println!("错误: {}", err);
                        } else {
                            println!("错误消息: {:?}", msg.payload);
                        }
                    }
                    _ => {
                         println!("\n收到未处理消息: {:?}", msg);
                    }
                }
            }
        }
    });

     println!("\n进入交互模式. 输入 'exit' 退出.");
     println!("发送消息格式: <目标Node_ID> <消息内容>");
    println!("查看在线节点: list");
    println!("查看服务器状态: status");

    // 交互式发送消息
    loop {
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let trimmed_input = input.trim();

        if trimmed_input.eq_ignore_ascii_case("exit") {
            break;
        }

        if trimmed_input.eq_ignore_ascii_case("list") {
            let list_req = Message::new(MessageType::ListNodesRequest, serde_json::Value::Null);
            if let Err(e) = send_message(&socket, &list_req, server_addr).await {
                println!("发送 list 请求失败: {}", e);
            }
            continue;
        }

        if trimmed_input.eq_ignore_ascii_case("status") {
            let state = client_state.lock().await;
            println!("服务器状态: {:?}", state.server_status);
            println!("P2P连接数: {}", p2p_peers.lock().await.len());
            println!("已知节点数: {}", peers.lock().await.len());
            if let Some(last_heartbeat) = state.last_server_heartbeat {
                println!("上次服务器心跳: {}秒前", last_heartbeat.elapsed().as_secs());
            } else {
                println!("尚未收到服务器心跳");
            }
            println!("心跳超时次数: {}", state.heartbeat_timeout_count);
            continue;
        }

        let parts: Vec<&str> = trimmed_input.splitn(2, ' ').collect();
        if parts.len() == 2 {
            if let Ok(dest_id) = Uuid::parse_str(parts[0]) {
                let content = parts[1];

                // 改进的消息发送逻辑：优先使用P2P连接
                let p2p_addr = p2p_peers.lock().await.get(&dest_id).cloned();
                let server_online = client_state.lock().await.is_server_online();

                let original = Message::data(serde_json::json!({"text": content}));
                let routed = RoutedMessage::new(original, own_node_id, dest_id, 8).to_message();

                let mut send_success = false;

                // 1. 优先尝试P2P直连
                if let Some(p2p_target_addr) = p2p_addr {
                    println!("尝试通过P2P连接发送消息到: {}", dest_id);
                    match send_message(&socket, &routed, p2p_target_addr).await {
                        Ok(_) => {
                            println!("消息已通过P2P发送 -> {}", dest_id);
                            send_success = true;
                        }
                        Err(e) => {
                            println!("P2P发送失败: {}, 尝试其他方式", e);
                            // P2P失败，移除无效连接
                            p2p_peers.lock().await.remove(&dest_id);
                        }
                    }
                }

                // 2. 如果P2P失败且服务器在线，通过服务器转发
                if !send_success && server_online {
                    println!("尝试通过服务器转发消息到: {}", dest_id);
                    
                    // 如果没有P2P连接，先尝试建立
                    if p2p_addr.is_none() {
                        let p2p_req = Message::initiate_p2p(dest_id);
                        let _ = send_message(&socket, &p2p_req, server_addr).await;
                    }

                    match send_message(&socket, &routed, server_addr).await {
                        Ok(_) => {
                            println!("消息已通过服务器转发 -> {}", dest_id);
                            send_success = true;
                        }
                        Err(e) => {
                            println!("服务器转发失败: {}", e);
                        }
                    }
                }

                // 3. 如果服务器离线，尝试通过已知节点地址直接发送
                if !send_success {
                    let peers_map = peers.lock().await;
                    if let Some(peer) = peers_map.get(&dest_id) {
                        println!("尝试直接发送到节点地址: {}", peer.addr);
                        match send_message(&socket, &routed, peer.addr).await {
                            Ok(_) => {
                                println!("消息已直接发送 -> {}", dest_id);
                                send_success = true;
                                // 成功后将此地址加入P2P连接
                                p2p_peers.lock().await.insert(dest_id, peer.addr);
                            }
                            Err(e) => {
                                println!("直接发送失败: {}", e);
                            }
                        }
                    }
                }

                if !send_success {
                    println!("消息发送失败: 所有发送方式都不可用");
                    println!("提示: 服务器状态={:?}, P2P连接={}, 已知节点={}",
                        client_state.lock().await.server_status,
                        p2p_peers.lock().await.contains_key(&dest_id),
                        peers.lock().await.contains_key(&dest_id)
                    );
                }
            } else {
                println!("无效的 Node ID 格式.");
            }
        } else {
            println!("输入格式错误. 请使用: <目标Node_ID> <消息内容>");
        }
    }
    
    // 结束示例
    let _ = send_message(&socket, &Message::disconnect("done".to_string()), server_addr).await;
    println!("客户端 [{}] 已断开连接.", client_name);

    Ok(())
}

async fn handshake(socket: &UdpSocket, server_addr: SocketAddr, node_info: NodeInfo) -> Result<Uuid> {
    let local_addr = node_info.listen_addr; // 从node_info获取地址
    let hs = Message::new_with_ack(MessageType::HandshakeRequest, serde_json::to_value(&node_info)?, local_addr, 1);
    send_message(socket, &hs, server_addr).await?;
    println!("已发送握手请求: {} -> {}", local_addr, server_addr);

    let handshake_timeout = Duration::from_secs(5);
    let start_time = tokio::time::Instant::now();

    loop {
        // 设置单次接收的超时
        match timeout(Duration::from_secs(1), receive_message(socket)).await {
            Ok(Ok(Some(resp))) => {
                match resp.message_type {
                    MessageType::HandshakeResponse => {
                        let hr: HandshakeResponse = serde_json::from_value(resp.payload.clone())?;
                        if hr.success {
                            println!(
                                "握手响应: 节点={} 成功={} 网络ID={}",
                                hr.node_info.name, hr.success, hr.node_info.network_id
                            );
                            return Ok(hr.node_info.id);
                        } else {
                            return Err(anyhow::anyhow!("服务器拒绝握手: {:?}", hr));
                        }
                    }
                    MessageType::Ack => {
                        println!("收到服务器的Ack确认，继续等待握手响应...");
                        // 忽略Ack，继续循环等待
                    }
                    MessageType::Error => {
                        return Err(anyhow::anyhow!("握手失败: {:?}", resp.payload));
                    }
                    _ => {
                        return Err(anyhow::anyhow!("收到意外消息类型: {:?}", resp.message_type));
                    }
                }
            }
            Ok(Ok(None)) => {
                // receive_message 内部超时，继续等待
            }
            Ok(Err(e)) => {
                return Err(e.into());
            }
            Err(_) => { // 单次接收超时
                 // 不做任何事，让外层循环检查总体超时
            }
        }

        if start_time.elapsed() > handshake_timeout {
            return Err(anyhow::anyhow!("握手响应超时: {}", local_addr));
        }
    }
}

async fn send_message(socket: &UdpSocket, message: &Message, target: SocketAddr) -> Result<()> {
    let data = serde_json::to_vec(message)?;
    socket.send_to(&data, target).await?;
    Ok(())
}

async fn receive_message(socket: &UdpSocket) -> Result<Option<Message>> {
    let mut buffer = vec![0u8; 65536];
    match timeout(Duration::from_secs(3600), socket.recv_from(&mut buffer)).await { // 延长超时以便持续接收
        Ok(Ok((len, addr))) => {
            buffer.truncate(len);
            let mut message: Message = serde_json::from_slice(&buffer)?;
            message.sender_addr = Some(addr);
            Ok(Some(message))
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Ok(None), // 超时
    }
}