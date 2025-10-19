use anyhow::Result;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use std::net::SocketAddr;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use p2p_handshake_server::protocol::{Message, MessageType, HandshakeResponse, NodeInfo, ListNodesResponse, PeerInfo};
use p2p_handshake_server::router::RoutedMessage;

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

    // 启动消息接收任务
    let recv_socket = socket.clone();
    let peers_clone = peers.clone();
    let p2p_peers_clone = p2p_peers.clone();
    tokio::spawn(async move {
        loop {
            if let Some(msg) = receive_message(&recv_socket).await.unwrap_or(None) {
                 match msg.message_type {
                    MessageType::Data => {
                        if let Ok(rm) = RoutedMessage::from_message(&msg) {
                            println!("
收到消息: [来源: {}] [内容: {}]", rm.source_node, rm.original_message.payload);
                        } else {
                            println!("
收到非路由数据: {:?}", msg.payload);
                        }
                    },
                    MessageType::DiscoveryResponse => {
                        if let Ok(peers_data) = serde_json::from_value::<Vec<PeerInfo>>(msg.payload) {
                            println!("--- 发现节点列表 (自动下发) ---");
                            let mut peers_map = peers_clone.lock().await;
                            for peer in peers_data {
                                println!("- ID: {}, 地址: {}", peer.id, peer.addr);
                                peers_map.insert(peer.id, peer);
                            }
                            println!("---------------------------");
                        } else {
                            println!("无法解析节点发现响应");
                        }
                    },
                    MessageType::ListNodesResponse => {
                        if let Ok(list_response) = serde_json::from_value::<ListNodesResponse>(msg.payload) {
                            println!("--- 在线节点列表 (手动刷新) ---");
                            let mut peers_map = peers_clone.lock().await;
                            for node in list_response.nodes {
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
                            println!("收到 P2P 连接指令，目标地址: {}，目标ID: {}", peer_addr, peer_id);
                            let mut p2p_peers_map = p2p_peers_clone.lock().await;
                            p2p_peers_map.insert(peer_id, peer_addr);

                            // Send a punch-through packet
                            let punch_msg = Message::ping();
                            let recv_socket_clone = recv_socket.clone();
                            tokio::spawn(async move {
                                if let Err(e) = send_message(&recv_socket_clone, &punch_msg, peer_addr).await {
                                    println!("打洞失败: {}", e);
                                }
                            });
                        }
                    },
                     MessageType::Ack => {
                         // 可以选择忽略或处理Ack
                    },
                    MessageType::Ping => {
                        // 收到Ping，回复Pong
                        if let Some(addr) = msg.sender_addr {
                            let pong = Message::new(MessageType::Pong, serde_json::Value::Null);
                            let recv_socket_clone = recv_socket.clone();
                            tokio::spawn(async move {
                                if let Err(e) = send_message(&recv_socket_clone, &pong, addr).await {
                                    println!("回复 Pong 失败: {}", e);
                                }
                            });
                        }
                    }
                    _ => {
                         println!("
收到未处理消息: {:?}", msg);
                    }
                }
            }
        }
    });

     println!("\n进入交互模式. 输入 'exit' 退出.");
     println!("发送消息格式: <目标Node_ID> <消息内容>");
    println!("查看在线节点: list");

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

        let parts: Vec<&str> = trimmed_input.splitn(2, ' ').collect();
        if parts.len() == 2 {
            if let Ok(dest_id) = Uuid::parse_str(parts[0]) {
                let content = parts[1];

                let p2p_addr = p2p_peers.lock().await.get(&dest_id).cloned();

                if p2p_addr.is_none() {
                    // Initiate P2P connection
                    let p2p_req = Message::initiate_p2p(dest_id);
                    if let Err(e) = send_message(&socket, &p2p_req, server_addr).await {
                        println!("发送 P2P 请求失败: {}", e);
                    }
                }

                 let original = Message::data(serde_json::json!({"text": content}));
                 let routed = RoutedMessage::new(original, own_node_id, dest_id, 8).to_message();

                 let target_addr = if let Some(addr) = p2p_addr {
                    addr
                 } else {
                    let peers_map = peers.lock().await;
                    if let Some(peer) = peers_map.get(&dest_id) {
                        peer.addr
                    } else {
                        // 如果在本地找不到，则通过服务器转发
                        server_addr
                    }
                 };

                 if let Err(e) = send_message(&socket, &routed, target_addr).await {
                     println!("发送失败: {}", e);
                } else {
                    println!("消息已发送 -> {}", dest_id);
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