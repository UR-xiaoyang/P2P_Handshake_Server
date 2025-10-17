use anyhow::Result;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration, sleep};
use std::net::SocketAddr;
use std::env;

use p2p_handshake_server::protocol::{Message, MessageType, HandshakeResponse, NodeInfo};
use p2p_handshake_server::router::RoutedMessage;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // 从命令行参数获取服务器地址，否则使用默认值
    let server_addr: SocketAddr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8080".to_string())
        .parse()?;

    println!("将连接到服务器: {}", server_addr);

    // 创建两个客户端
    let client1 = UdpSocket::bind("0.0.0.0:0").await?;
    let client2 = UdpSocket::bind("0.0.0.0:0").await?;
    let client1_addr = client1.local_addr()?;
    let client2_addr = client2.local_addr()?;

    println!("客户端1地址: {}", client1_addr);
    println!("客户端2地址: {}", client2_addr);

    // 准备节点信息（network_id 固定为 test）
    let client1_info = NodeInfo::new("client1".to_string(), client1_addr, "test".to_string());
    let client2_info = NodeInfo::new("client2".to_string(), client2_addr, "test".to_string());

    // 并发进行握手
    let h1 = handshake_and_print(&client1, client1_addr, server_addr, client1_info.clone());
    let h2 = handshake_and_print(&client2, client2_addr, server_addr, client2_info.clone());
    let _ = tokio::join!(h1, h2);

    // 给服务端一点时间更新路由表
    sleep(Duration::from_millis(200)).await;

    // 客户端1 -> 客户端2 发送“成功连接到节点”
    let original1 = Message::data(serde_json::json!({"text": "成功连接到节点"}));
    let routed1 = RoutedMessage::new(original1, client1_info.id, client2_info.id, 8).to_message();
    send_message(&client1, &routed1, server_addr).await?;
    if let Some(msg) = receive_message(&client2).await? {
        if msg.message_type == MessageType::Data {
            if let Ok(rm) = RoutedMessage::from_message(&msg) {
                println!(
                    "客户端2收到: 路由ID={:?} 来源={} 目标={} 原始数据={:?}",
                    rm.route_id, rm.source_node, rm.destination_node, rm.original_message.payload
                );
            } else {
                println!("客户端2收到非路由数据: {:?}", msg.payload);
            }
        }
    } else {
        println!("客户端2接收数据超时");
    }

    // 客户端2 -> 客户端1 发送“成功连接到节点”
    let original2 = Message::data(serde_json::json!({"text": "成功连接到节点"}));
    let routed2 = RoutedMessage::new(original2, client2_info.id, client1_info.id, 8).to_message();
    send_message(&client2, &routed2, server_addr).await?;
    if let Some(msg) = receive_message(&client1).await? {
        if msg.message_type == MessageType::Data {
            if let Ok(rm) = RoutedMessage::from_message(&msg) {
                println!(
                    "客户端1收到: 路由ID={:?} 来源={} 目标={} 原始数据={:?}",
                    rm.route_id, rm.source_node, rm.destination_node, rm.original_message.payload
                );
            } else {
                println!("客户端1收到非路由数据: {:?}", msg.payload);
            }
        }
    } else {
        println!("客户端1接收数据超时");
    }

    // 结束示例
    let _ = send_message(&client1, &Message::disconnect("done".to_string()), server_addr).await;
    let _ = send_message(&client2, &Message::disconnect("done".to_string()), server_addr).await;

    println!("\nP2P 互发测试完成");
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
                println!(
                    "握手响应: 节点={} 成功={} 网络ID={}",
                    hr.node_info.name, hr.success, hr.node_info.network_id
                );
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