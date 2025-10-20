use std::time::Duration;
use tokio::time::sleep;
use p2p_handshake_server::{Config, P2PServer};
use p2p_handshake_server::network::NetworkManager;
use p2p_handshake_server::protocol::{Message, NodeInfo};
use log::{info, warn};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    // 创建测试配置，使用较短的超时时间便于测试
    let mut config = Config::default();
    config.listen_address = "127.0.0.1:8080".parse().unwrap();
    config.heartbeat_interval = 5; // 5秒心跳间隔
    config.connection_timeout = 10; // 10秒超时
    config.network_id = "test_cleanup".to_string();
    
    info!("启动P2P服务器，心跳间隔: {}秒，超时时间: {}秒", 
          config.heartbeat_interval, config.connection_timeout);
    
    // 启动服务器
    let mut server = P2PServer::new(config.clone()).await?;
    let server_handle = tokio::spawn(async move {
        if let Err(e) = server.run().await {
            warn!("服务器运行错误: {}", e);
        }
    });
    
    // 等待服务器启动
    sleep(Duration::from_secs(1)).await;
    
    // 创建一个正常的客户端（会响应ping）
    info!("创建正常客户端...");
    let normal_client = NetworkManager::new("127.0.0.1:0".parse().unwrap()).await?;
    let normal_node_info = NodeInfo::new(
        "normal_client".to_string(),
        normal_client.local_addr(),
        config.network_id.clone(),
    );
    
    // 发送握手请求
    let handshake_msg = Message::handshake_request(normal_node_info.clone());
    normal_client.send_to(&handshake_msg, config.listen_address).await?;
    
    // 接收握手响应
    let (data, _) = normal_client.receive_from().await?;
    let response = normal_client.parse_message(&data)?;
    info!("正常客户端握手成功: {:?}", response.message_type);
    
    // 创建一个"僵尸"客户端（不会响应ping）
    info!("创建僵尸客户端（不响应ping）...");
    let zombie_client = NetworkManager::new("127.0.0.1:0".parse().unwrap()).await?;
    let zombie_node_info = NodeInfo::new(
        "zombie_client".to_string(),
        zombie_client.local_addr(),
        config.network_id.clone(),
    );
    
    // 发送握手请求
    let handshake_msg = Message::handshake_request(zombie_node_info.clone());
    zombie_client.send_to(&handshake_msg, config.listen_address).await?;
    
    // 接收握手响应
    let (data, _) = zombie_client.receive_from().await?;
    let response = zombie_client.parse_message(&data)?;
    info!("僵尸客户端握手成功: {:?}", response.message_type);
    
    info!("两个客户端已连接，开始测试...");
    
    // 正常客户端处理消息循环
    let normal_client_handle = {
        let client = normal_client;
        tokio::spawn(async move {
            loop {
                match client.receive_from().await {
                    Ok((data, sender)) => {
                        if let Ok(msg) = client.parse_message(&data) {
                            match msg.message_type {
                                p2p_handshake_server::protocol::MessageType::Ping => {
                                    info!("正常客户端收到ping，发送pong");
                                    let pong = Message::pong();
                                    if let Err(e) = client.send_to(&pong, sender).await {
                                        warn!("发送pong失败: {}", e);
                                    }
                                }
                                _ => {
                                    info!("正常客户端收到其他消息: {:?}", msg.message_type);
                                }
                            }
                        }
                    }
                    Err(_) => {
                        // 接收错误，继续循环
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }
        })
    };
    
    // 僵尸客户端不处理任何消息（模拟不响应的节点）
    info!("僵尸客户端将不响应任何ping消息");
    
    // 等待足够长的时间让服务器检测到僵尸客户端超时
    info!("等待 20 秒，观察服务器是否会清理僵尸客户端...");
    sleep(Duration::from_secs(20)).await;
    
    info!("测试完成");
    
    // 清理
    normal_client_handle.abort();
    server_handle.abort();
    
    Ok(())
}