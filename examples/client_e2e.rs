use anyhow::Result;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration, sleep};
use std::net::SocketAddr;

use p2p_handshake_server::{Config, P2PServer};
use p2p_handshake_server::protocol::{Message, MessageType, HandshakeResponse, NodeInfo, PeerInfo};
use p2p_handshake_server::router::RoutedMessage;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // 配置服务器，确保 network_id 与客户端一致
    let mut config = Config::default();
    config.network_id = "test".to_string();
    config.listen_address = "127.0.0.1:8080".parse().unwrap();

    // 启动服务器（在同一进程内）
    let mut server = P2PServer::new(config.clone()).await?;
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            eprintln!("服务器运行失败: {}", e);
        }
    });

    // 等待服务器就绪
    sleep(Duration::from_millis(300)).await;

    let server_addr: SocketAddr = config.listen_address;

    // 创建两个客户端
    let client1 = UdpSocket::bind("127.0.0.1:0").await?;
    let client2 = UdpSocket::bind("127.0.0.1:0").await?;
    let client1_addr = client1.local_addr()?;
    let client2_addr = client2.local_addr()?;

    println!("客户端1地址: {}", client1_addr);
    println!("客户端2地址: {}", client2_addr);

    // 准备节点信息（与服务器 network_id 一致）
    let client1_info = NodeInfo::new("client1".to_string(), client1_addr, "test".to_string());
    let client2_info = NodeInfo::new("client2".to_string(), client2_addr, "test".to_string());

    // 并发进行握手
    let h1 = handshake_and_print(&client1, client1_addr, server_addr, client1_info.clone());
    let h2 = handshake_and_print(&client2, client2_addr, server_addr, client2_info.clone());
    let _ = tokio::join!(h1, h2);

    // 握手完成后，主动发送 DiscoveryRequest，以便拉取当前分发的节点列表
    send_message(&client1, &Message::discovery_request(), server_addr).await?;
    send_message(&client2, &Message::discovery_request(), server_addr).await?;

    // 接收并打印分发的节点列表
    println!("\n开始接收并打印节点分发结果（一次性）...");
    receive_and_print_discovery(&client1, client1_addr, "客户端1").await?;
    receive_and_print_discovery(&client2, client2_addr, "客户端2").await?;

    // Ping/Pong 测试
    let ping = Message::new_with_ack(MessageType::Ping, serde_json::Value::Null, client1_addr, 99);
    send_message(&client1, &ping, server_addr).await?;
    if let Some(pong) = receive_message(&client1).await? {
        if pong.message_type == MessageType::Pong { println!("客户端1收到Pong"); }
    }

    // 客户端1 -> 客户端2 路由数据测试
    println!("\n客户端1向客户端2发送路由数据...");
    let original = Message::data(serde_json::json!({"msg": "hello_from_client1"}));
    let routed = RoutedMessage::new(original, client1_info.id, client2_info.id, 8).to_message();
    send_message(&client1, &routed, server_addr).await?;
    if let Some(msg) = receive_message(&client2).await? {
        if msg.message_type == MessageType::Data {
            if let Ok(rm) = p2p_handshake_server::router::RoutedMessage::from_message(&msg) {
                println!("客户端2收到路由消息: route_id={:?}, 来源={}, 目标={}", rm.route_id, rm.source_node, rm.destination_node);
                println!("原始数据: {:?}", rm.original_message.payload);
            } else {
                println!("客户端2收到非路由数据: {:?}", msg.payload);
            }
        }
    }

    // 客户端1请求服务器返回路由表快照
    println!("\n客户端1请求获取路由快照...");
    let cmd_get_routes = Message::data(serde_json::json!({"cmd": "get_routes"}));
    send_message(&client1, &cmd_get_routes, server_addr).await?;
    {
        // 在一个窗口内尝试接收包含 routes 的数据返回
        let window = Duration::from_millis(2000);
        let deadline = tokio::time::Instant::now() + window;
        let mut printed = false;
        while tokio::time::Instant::now() < deadline {
            if let Ok(Ok(Some(resp))) = timeout(Duration::from_millis(300), receive_message(&client1)).await {
                if resp.message_type == MessageType::Data {
                    if let Some(obj) = resp.payload.as_object() {
                        if let Some(routes_val) = obj.get("routes") {
                            println!("路由快照: {} 项", routes_val.as_array().map(|a| a.len()).unwrap_or(0));
                            println!("{}", routes_val);
                            printed = true;
                            break;
                        }
                    }
                }
            }
        }
        if !printed { println!("在窗口内未获取到路由快照响应"); }
    }

    // 结束示例，发送断开消息（非必须）
    let _ = send_message(&client1, &Message::disconnect("done".to_string()), server_addr).await;
    let _ = send_message(&client2, &Message::disconnect("done".to_string()), server_addr).await;

    // 停止服务器任务
    server_handle.abort();
    println!("\n完整客户端模拟完成");
    Ok(())
}

async fn handshake_and_print(socket: &UdpSocket, local_addr: SocketAddr, server_addr: SocketAddr, node_info: NodeInfo) -> Result<()> {
    let hs = Message::new_with_ack(MessageType::HandshakeRequest, serde_json::to_value(&node_info)?, local_addr, 1);
    send_message(socket, &hs, server_addr).await?;
    println!("已发送握手请求: {} -> {}", local_addr, server_addr);

    if let Some(resp) = receive_message(socket).await? {
        match resp.message_type {
            MessageType::HandshakeResponse => {
                let hr: HandshakeResponse = serde_json::from_value(resp.payload.clone())?;
                println!("握手响应: 节点={} 成功={} 网络ID={}", hr.node_info.name, hr.success, hr.node_info.network_id);
            }
            MessageType::Error => {
                println!("握手失败: {:?}", resp.payload);
            }
            _ => {
                println!("收到意外消息类型: {:?}", resp.message_type);
            }
        }
    } else {
        println!("握手响应超时: {}", local_addr);
    }
    Ok(())
}

async fn receive_and_print_discovery(socket: &UdpSocket, local_addr: SocketAddr, who: &str) -> Result<()> {
    let window = Duration::from_millis(800);
    let deadline = tokio::time::Instant::now() + window;
    let mut latest: Option<Vec<PeerInfo>> = None;
    loop {
        if tokio::time::Instant::now() >= deadline { break; }
        match timeout(Duration::from_millis(300), receive_message(socket)).await {
            Ok(Ok(Some(msg))) => {
                if msg.message_type == MessageType::DiscoveryResponse {
                    let mut peers: Vec<PeerInfo> = serde_json::from_value(msg.payload.clone())?;
                    peers.retain(|p| p.addr != local_addr);
                    latest = Some(peers);
                }
            }
            _ => {}
        }
    }

    let peers = latest.unwrap_or_default();
    println!("{} 收到节点列表 ({} 条):", who, peers.len());
    for p in peers {
        println!("  - id={} addr={} caps={:?}", p.id, p.addr, p.capabilities);
    }
    Ok(())
}

async fn send_message(socket: &UdpSocket, message: &Message, target: SocketAddr) -> Result<()> {
    let data = serde_json::to_vec(message)?;
    socket.send_to(&data, target).await?;
    Ok(())
}

async fn receive_message(socket: &UdpSocket) -> Result<Option<Message>> {
    let mut buffer = vec![0u8; 65536];
    match timeout(Duration::from_secs(3), socket.recv_from(&mut buffer)).await {
        Ok(Ok((len, _addr))) => {
            buffer.truncate(len);
            let message: Message = serde_json::from_slice(&buffer)?;
            Ok(Some(message))
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Ok(None),
    }
}