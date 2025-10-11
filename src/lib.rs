//! # P2P握手服务器
//! 
//! 这是一个用Rust编写的P2P网络握手服务器实现。
//! 
//! ## 功能特性
//! 
//! - TCP连接管理
//! - P2P节点发现和握手协议
//! - 消息路由和转发
//! - 节点管理和连接池
//! - 配置文件支持
//! - 完整的日志记录
//! 
//! ## 使用示例
//! 
//! ```rust,no_run
//! use p2p_handshake_server::{Config, P2PServer};
//! 
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let config = Config::default();
//!     let mut server = P2PServer::new(config).await?;
//!     server.run().await?;
//!     Ok(())
//! }
//! ```

pub mod config;
pub mod network;
pub mod peer;
pub mod protocol;
pub mod router;
pub mod server;

// 重新导出主要的公共API
pub use config::Config;
pub use server::P2PServer;
pub use protocol::{Message, MessageType, NodeInfo};
pub use peer::{Peer, PeerManager, PeerStatus};
pub use network::{Connection, NetworkManager};
pub use router::{MessageRouter, RoutedMessage, RoutingTable};