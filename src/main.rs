use log::{info, error};
use log::LevelFilter;
use clap::{Parser, ArgAction};
use clap::ArgGroup;

mod network;
mod peer;
mod protocol;
mod server;
mod config;
mod router;
mod stun_server;
mod stun_protocol;

use crate::server::P2PServer;
use crate::config::Config;

#[derive(Parser)]
#[command(name = "p2p_server")]
#[command(about = "A P2P network handshake server")]
#[command(group(
    ArgGroup::new("log_level")
        .args(["trace", "debug", "info", "warn", "error"]) 
        .multiple(false)
))]
struct Args {
    /// 服务器监听地址
    #[arg(short, long)]
    address: Option<std::net::SocketAddr>,
    
    /// 最大连接数
    #[arg(short, long)]
    max_connections: Option<usize>,
    
    /// 配置文件路径
    #[arg(short, long)]
    config: Option<String>,

    /// 网络ID
    #[arg(long)]
    network_id: Option<String>,

    /// 心跳间隔（秒）
    #[arg(long)]
    heartbeat_interval: Option<u64>,

    /// 连接超时时间（秒）
    #[arg(long)]
    connection_timeout: Option<u64>,

    /// 是否启用节点发现
    #[arg(long)]
    enable_discovery: Option<bool>,

    /// 是否启用内置STUN服务器
    #[arg(long = "STUN", action = ArgAction::SetTrue)]
    enable_stun: bool,

    /// 启用流量转发功能
    #[arg(long = "relay", action = ArgAction::SetTrue)]
    enable_relay: bool,

    /// 设置日志级别为 TRACE
    #[arg(long = "TRACE", action = ArgAction::SetTrue)]
    trace: bool,
    /// 设置日志级别为 DEBUG
    #[arg(long = "DEBUG", action = ArgAction::SetTrue)]
    debug: bool,
    /// 设置日志级别为 INFO
    #[arg(long = "INFO", action = ArgAction::SetTrue)]
    info: bool,
    /// 设置日志级别为 WARN
    #[arg(long = "WARN", action = ArgAction::SetTrue)]
    warn: bool,
    /// 设置日志级别为 ERROR
    #[arg(long = "ERROR", action = ArgAction::SetTrue)]
    error: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 解析命令行参数，并根据日志级别初始化日志
    let args = Args::parse();

    let explicit_level = if args.trace {
        Some(LevelFilter::Trace)
    } else if args.debug {
        Some(LevelFilter::Debug)
    } else if args.warn {
        Some(LevelFilter::Warn)
    } else if args.error {
        Some(LevelFilter::Error)
    } else if args.info {
        Some(LevelFilter::Info)
    } else {
        None
    };

    if let Some(level) = explicit_level {
        env_logger::Builder::from_default_env()
            .filter_level(level)
            .init();
    } else {
        // 未指定日志级别时，使用环境变量或默认级别
        env_logger::Builder::from_default_env().init();
    }

    info!("启动P2P握手服务器...");
    
    // 确定基础配置：优先从文件加载，否则使用默认值
    let mut config = if let Some(config_path) = args.config {
        Config::from_file(&config_path)?
    } else {
        Config::default()
    };

    // 使用命令行参数覆盖配置
    if let Some(address) = args.address {
        config.listen_address = address;
    }
    if let Some(max_connections) = args.max_connections {
        config.max_connections = max_connections;
    }
    if let Some(network_id) = args.network_id {
        config.network_id = network_id;
    }
    if let Some(heartbeat_interval) = args.heartbeat_interval {
        config.heartbeat_interval = heartbeat_interval;
    }
    if let Some(connection_timeout) = args.connection_timeout {
        config.connection_timeout = connection_timeout;
    }
    if let Some(enable_discovery) = args.enable_discovery {
        config.enable_discovery = enable_discovery;
    }

    // 处理STUN服务器启用参数
    if args.enable_stun {
        config.stun_server.enable = true;
    }

    // 处理流量转发参数
    if args.enable_relay {
        config.allow_symmetric_nat_relay = true;
    }

    info!("最终配置: {:?}", config);

    // 创建并启动服务器
    let mut server = P2PServer::new(config.clone()).await?;
    
    info!("服务器正在监听地址: {}", config.listen_address);
    
    // 启动服务器
    if let Err(e) = server.run().await {
        error!("服务器运行错误: {}", e);
        return Err(e);
    }
    
    Ok(())
}