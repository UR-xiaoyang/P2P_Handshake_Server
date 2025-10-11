use tokio::net::UdpSocket;
use anyhow::{Result, Context};
use serde_json;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::time::timeout;

use p2p_handshake_server::protocol::{NodeInfo, Message, MessageType};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let server_addr: SocketAddr = "127.0.0.1:8080".parse()?;
    let client_addr: SocketAddr = "127.0.0.1:0".parse()?;
    
    println!("创建UDP客户端，连接到服务器: {}", server_addr);
    
    // 创建UDP套接字
    let socket = UdpSocket::bind(client_addr).await
        .context("绑定UDP套接字失败")?;
    
    let local_addr = socket.local_addr()?;
    println!("客户端绑定到: {}", local_addr);
    
    // 创建客户端节点信息
    let mut client_info = NodeInfo::new("test_client".to_string(), local_addr);
    client_info.add_capability("test".to_string());
    client_info.add_metadata("client_type".to_string(), "udp_test_client".to_string());
    
    // 发送握手请求
    let handshake_request = Message::new_with_ack(
        MessageType::HandshakeRequest,
        serde_json::to_value(&client_info)?,
        local_addr,
        1, // 序列号
    );
    send_message(&socket, &handshake_request, server_addr).await?;
    println!("已发送握手请求");
    
    // 接收握手响应
    if let Some(response) = receive_message(&socket).await? {
        match response.message_type {
            MessageType::HandshakeResponse => {
                println!("收到握手响应: {:?}", response.payload);
            }
            MessageType::Error => {
                println!("握手失败: {:?}", response.payload);
                return Ok(());
            }
            _ => {
                println!("收到意外消息类型: {:?}", response.message_type);
            }
        }
    }
    
    // 发送一些测试数据
    let test_data = serde_json::json!({
        "message": "Hello from UDP client!",
        "timestamp": chrono::Utc::now().timestamp()
    });
    
    let data_message = Message::new_with_ack(
        MessageType::Data,
        test_data,
        local_addr,
        2, // 序列号
    );
    send_message(&socket, &data_message, server_addr).await?;
    println!("已发送测试数据");
    
    // 发送ping
    let ping_message = Message::new_with_ack(
        MessageType::Ping,
        serde_json::Value::Null,
        local_addr,
        3, // 序列号
    );
    send_message(&socket, &ping_message, server_addr).await?;
    println!("已发送ping");
    
    // 接收pong
    if let Some(pong) = receive_message(&socket).await? {
        if pong.message_type == MessageType::Pong {
            println!("收到pong响应");
        }
    }
    
    // 发送断开连接消息
    let disconnect_message = Message::new(
        MessageType::Disconnect,
        serde_json::json!({"reason": "客户端主动断开"})
    );
    send_message(&socket, &disconnect_message, server_addr).await?;
    println!("已发送断开连接消息");
    
    println!("UDP客户端测试完成");
    Ok(())
}

async fn send_message(socket: &UdpSocket, message: &Message, target: SocketAddr) -> Result<()> {
    let data = serde_json::to_vec(message)?;
    socket.send_to(&data, target).await?;
    Ok(())
}

async fn receive_message(socket: &UdpSocket) -> Result<Option<Message>> {
    let mut buffer = vec![0u8; 65536]; // UDP最大包大小
    
    match timeout(Duration::from_secs(5), socket.recv_from(&mut buffer)).await {
        Ok(Ok((len, _addr))) => {
            buffer.truncate(len);
            let message: Message = serde_json::from_slice(&buffer)?;
            Ok(Some(message))
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => {
            println!("接收消息超时");
            Ok(None)
        }
    }
}