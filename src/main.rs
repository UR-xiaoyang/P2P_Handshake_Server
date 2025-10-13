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
    #[arg(short, long, default_value = "127.0.0.1:8080")]
    address: std::net::SocketAddr,
    
    /// 最大连接数
    #[arg(short, long, default_value = "100")]
    max_connections: usize,
    
    /// 配置文件路径
    #[arg(short, long)]
    config: Option<String>,

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
    
    // 加载配置
    let config = if let Some(config_path) = args.config.clone() {
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