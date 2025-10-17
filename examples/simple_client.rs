use anyhow::Result;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use std::net::SocketAddr;
use std::env;
use std::sync::Arc;
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


    // 启动消息接收任务
    let recv_socket = socket.clone();
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
                        if let Ok(peers) = serde_json::from_value::<Vec<PeerInfo>>(msg.payload) {
                            println!("--- 发现节点列表 (自动下发) ---");
                            for peer in peers {
                                println!("- ID: {}, 地址: {}", peer.id, peer.addr);
                            }
                            println!("---------------------------");
                        } else {
                            println!("无法解析节点发现响应");
                        }
                    },
                    MessageType::ListNodesResponse => {
                        if let Ok(list_response) = serde_json::from_value::<ListNodesResponse>(msg.payload) {
                            println!("--- 在线节点列表 (手动刷新) ---");
                            for node in list_response.nodes {
                                println!("- ID: {}, 名称: {}, 地址: {}", node.id, node.name, node.listen_addr);
                            }
                            println!("---------------------");
                        } else {
                            println!("无法解析节点列表响应");
                        }
                    },
                    MessageType::Ack => {
                        // 可以选择忽略或处理Ack
                    },
                    _ => {
                         println!("
收到未处理消息: {:?}", msg);
                    }
                }
            }
        }
    });

    println!("
进入交互模式. 输入 'exit' 退出.");
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
                let original = Message::data(serde_json::json!({"text": content}));
                let routed = RoutedMessage::new(original, own_node_id, dest_id, 8).to_message();
                if let Err(e) = send_message(&socket, &routed, server_addr).await {
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
        Ok(Ok((len, _addr))) => {
            buffer.truncate(len);
            let message: Message = serde_json::from_slice(&buffer)?;
            Ok(Some(message))
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Ok(None), // 超时
    }
}