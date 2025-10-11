use clap::Parser;
use log::{info, error};
use std::net::SocketAddr;

mod network;
mod peer;
mod protocol;
mod server;
mod config;
mod router;

use crate::server::P2PServer;
use crate::config::Config;

#[derive(Parser)]
#[command(name = "p2p_server")]
#[command(about = "A P2P network handshake server")]
struct Args {
    /// 服务器监听地址
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    address: SocketAddr,
    
    /// 最大连接数
    #[arg(short, long, default_value = "100")]
    max_connections: usize,
    
    /// 配置文件路径
    #[arg(short, long)]
    config: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    env_logger::init();
    
    let args = Args::parse();
    
    info!("启动P2P握手服务器...");
    
    // 加载配置
    let config = if let Some(config_path) = args.config {
        Config::from_file(&config_path)?
    } else {
        Config::new(args.address, args.max_connections)
    };
    
    // 创建并启动服务器
    let mut server = P2PServer::new(config).await?;
    
    info!("服务器正在监听地址: {}", args.address);
    
    // 启动服务器
    if let Err(e) = server.run().await {
        error!("服务器运行错误: {}", e);
        return Err(e);
    }
    
    Ok(())
}