use anyhow::{Result, Context};
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use std::net::SocketAddr;
use uuid::Uuid;

use p2p_handshake_server::protocol::{Message, MessageType, HandshakeResponse};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // 简单参数解析：--server <addr> --network-id <id>
    let mut server_addr: SocketAddr = "127.0.0.1:8080".parse().expect("默认地址解析失败");
    let mut network_id: String = "p2p_default".to_string();
    let args: Vec<String> = std::env::args().collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--server" => {
                if i + 1 < args.len() { server_addr = args[i+1].parse().context("--server 地址解析失败")?; i += 1; }
            }
            "--network-id" => {
                if i + 1 < args.len() { network_id = args[i+1].clone(); i += 1; }
            }
            _ => {}
        }
        i += 1;
    }

    println!("诊断目标服务器: {} | 网络ID: {}", server_addr, network_id);

    // 生成一个固定的测试节点ID，用于两个客户端复用，验证服务器是否做去重
    let test_node_id = Uuid::new_v4();
    println!("使用测试节点ID: {}", test_node_id);

    // 客户端1
    let client1 = UdpSocket::bind("127.0.0.1:0").await.context("客户端1绑定失败")?;
    let client1_addr = client1.local_addr()?;
    println!("客户端1绑定到: {}", client1_addr);

    // 客户端2
    let client2 = UdpSocket::bind("127.0.0.1:0").await.context("客户端2绑定失败")?;
    let client2_addr = client2.local_addr()?;
    println!("客户端2绑定到: {}", client2_addr);

    // 构造带自定义ID的握手请求载荷（直接用JSON确保ID被服务器解析）
    let hs1_payload = serde_json::json!({
        "id": test_node_id,
        "name": "diag_client_1",
        "version": "diag",
        "listen_addr": client1_addr.to_string(),
        "capabilities": ["diagnostic"],
        "metadata": {"purpose": "node_id_check"},
        "network_id": network_id,
    });

    let hs2_payload = serde_json::json!({
        "id": test_node_id, // 与客户端1相同，触发服务器重复ID检查
        "name": "diag_client_2",
        "version": "diag",
        "listen_addr": client2_addr.to_string(),
        "capabilities": ["diagnostic"],
        "metadata": {"purpose": "node_id_check"},
        "network_id": network_id,
    });

    // 发送握手（客户端1）
    let hs1 = Message::new_with_ack(MessageType::HandshakeRequest, hs1_payload, client1_addr, 1);
    send_message(&client1, &hs1, server_addr).await?;
    println!("客户端1已发送握手请求");

    // 接收握手响应（客户端1）
    if let Some(resp1) = receive_message(&client1).await? {
        match resp1.message_type {
            MessageType::HandshakeResponse => {
                let hr: HandshakeResponse = serde_json::from_value(resp1.payload.clone())?;
                println!(
                    "客户端1握手成功 | 服务器节点ID={} | 回显网络ID={}",
                    hr.node_info.id, hr.node_info.network_id
                );
            }
            MessageType::Error => {
                println!("客户端1握手失败: {:?}", resp1.payload);
                // 如果是网络ID不匹配，直接给出判定
                if let Some(err) = resp1.payload.get("error") {
                    if err.as_str().unwrap_or("").contains("网络ID不匹配") {
                        println!("诊断结果: 客户端配置的网络ID与服务器不一致（客户端问题或配置问题）");
                    }
                }
                return Ok(());
            }
            other => {
                println!("客户端1收到意外消息类型: {:?}", other);
                return Ok(());
            }
        }
    } else {
        println!("客户端1接收握手响应超时");
        println!("诊断结果: 服务器未响应或网络异常（需要检查服务器是否运行/端口是否正确）");
        return Ok(());
    }

    // 发送握手（客户端2，复用同一个节点ID）
    let hs2 = Message::new_with_ack(MessageType::HandshakeRequest, hs2_payload, client2_addr, 2);
    send_message(&client2, &hs2, server_addr).await?;
    println!("客户端2已发送握手请求（同一节点ID，期望被拒绝）");

    if let Some(resp2) = receive_message(&client2).await? {
        match resp2.message_type {
            MessageType::HandshakeResponse => {
                let hr: HandshakeResponse = serde_json::from_value(resp2.payload.clone())?;
                println!(
                    "客户端2握手被接受 | 服务器节点ID={} | 网络ID={}",
                    hr.node_info.id, hr.node_info.network_id
                );
                println!("诊断结果: 服务器未正确拒绝重复节点ID（服务器端问题）");
            }
            MessageType::Error => {
                let err_msg = resp2.payload.get("error").and_then(|v| v.as_str()).unwrap_or("");
                println!("客户端2握手失败（预期）: {}", err_msg);
                if err_msg.contains("节点ID") && err_msg.contains("已存在") {
                    println!("诊断结果: 服务器已正确识别并拒绝重复节点ID（客户端设置ID成功，问题不在服务器）");
                } else if err_msg.contains("网络ID不匹配") {
                    println!("诊断结果: 网络ID不匹配导致失败（客户端或配置问题）");
                } else {
                    println!("诊断结果: 服务器返回错误，但非重复ID相关，需进一步查看服务器日志");
                }
            }
            other => {
                println!("客户端2收到意外消息类型: {:?}", other);
            }
        }
    } else {
        println!("客户端2接收握手响应超时");
        println!("诊断结果: 服务器未响应或网络异常（需要检查服务器是否运行/端口是否正确）");
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
    match timeout(Duration::from_secs(5), socket.recv_from(&mut buffer)).await {
        Ok(Ok((len, _))) => {
            buffer.truncate(len);
            let message: Message = serde_json::from_slice(&buffer)?;
            Ok(Some(message))
        }
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Ok(None),
    }
}