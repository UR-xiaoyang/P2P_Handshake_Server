use anyhow::Result;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration, sleep};
use std::net::SocketAddr;

use p2p_handshake_server::{Config, P2PServer};
use p2p_handshake_server::protocol::{Message, MessageType, HandshakeResponse, NodeInfo};
use uuid::Uuid;

async fn send_message(socket: &UdpSocket, message: &Message, target: SocketAddr) -> Result<()> {
    let data = serde_json::to_vec(message)?;
    socket.send_to(&data, target).await?;
    Ok(())
}

async fn receive_message(socket: &UdpSocket) -> Result<Option<Message>> {
    let mut buffer = vec![0u8; 65536];
    match timeout(Duration::from_secs(2), socket.recv_from(&mut buffer)).await {
        Ok(Ok((len, _addr))) => {
            buffer.truncate(len);
            let message: Message = serde_json::from_slice(&buffer)?;
            Ok(Some(message))
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Ok(None),
    }
}

#[tokio::test]
async fn test_reconnect_same_node_id() -> Result<()> {
    // 初始化日志（忽略重复初始化错误）
    let _ = env_logger::try_init();

    // 启动服务器在固定端口，避免 8080 冲突
    let mut config = Config::default();
    config.network_id = "test".to_string();
    config.listen_address = "127.0.0.1:18080".parse().unwrap();

    let mut server = P2PServer::new(config.clone()).await?;
    let server_handle = tokio::spawn(async move {
        let _ = server.run().await;
    });

    // 等待服务器就绪
    sleep(Duration::from_millis(200)).await;

    let server_addr = config.listen_address;

    // 客户端1与客户端2，使用同一个固定的节点ID
    let fixed_id = Uuid::new_v4();

    let client1 = UdpSocket::bind("127.0.0.1:0").await?;
    let client2 = UdpSocket::bind("127.0.0.1:0").await?;
    let client1_addr = client1.local_addr()?;
    let client2_addr = client2.local_addr()?;

    // 客户端1握手
    let mut client1_info = NodeInfo::new("client_reconnect".to_string(), client1_addr, "test".to_string());
    client1_info.id = fixed_id; // 强制使用固定ID
    let hs1 = Message::new_with_ack(MessageType::HandshakeRequest, serde_json::to_value(&client1_info)?, client1_addr, 1);
    send_message(&client1, &hs1, server_addr).await?;
    if let Some(resp1) = receive_message(&client1).await? {
        match resp1.message_type {
            MessageType::HandshakeResponse => {
                let hr: HandshakeResponse = serde_json::from_value(resp1.payload.clone())?;
                assert!(hr.success, "首次握手应该成功");
            }
            MessageType::Error => panic!("首次握手返回错误: {:?}", resp1.payload),
            _ => {}
        }
    } else {
        panic!("首次握手未在超时内收到响应");
    }

    // 不发送 Disconnect，直接用客户端2以相同ID进行重连握手（模拟下线又上线但服务器未及时清理旧状态）
    let mut client2_info = NodeInfo::new("client_reconnect".to_string(), client2_addr, "test".to_string());
    client2_info.id = fixed_id; // 使用相同ID
    let hs2 = Message::new_with_ack(MessageType::HandshakeRequest, serde_json::to_value(&client2_info)?, client2_addr, 2);
    send_message(&client2, &hs2, server_addr).await?;
    if let Some(resp2) = receive_message(&client2).await? {
        match resp2.message_type {
            MessageType::HandshakeResponse => {
                let hr: HandshakeResponse = serde_json::from_value(resp2.payload.clone())?;
                assert!(hr.success, "同ID重连握手应该成功而非报错");
            }
            MessageType::Error => panic!("同ID重连握手返回错误: {:?}", resp2.payload),
            _ => {}
        }
    } else {
        panic!("同ID重连握手未在超时内收到响应");
    }

    // 清理：停止服务器任务
    server_handle.abort();
    Ok(())
}