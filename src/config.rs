use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// 服务器监听地址
    pub listen_address: SocketAddr,
    
    /// 最大连接数
    pub max_connections: usize,
    
    /// 心跳间隔（秒）
    pub heartbeat_interval: u64,
    
    /// 连接超时时间（秒）
    pub connection_timeout: u64,
    
    /// 节点发现端口范围
    pub discovery_port_range: (u16, u16),
    
    /// 是否启用节点发现
    pub enable_discovery: bool,

    /// 网络ID（用于网络隔离与校验）
    pub network_id: String,
}

impl Config {
    pub fn new(listen_address: SocketAddr, max_connections: usize) -> Self {
        Self {
            listen_address,
            max_connections,
            heartbeat_interval: 30,
            connection_timeout: 60,
            discovery_port_range: (8081, 8090),
            enable_discovery: true,
            network_id: "p2p_default".to_string(),
        }
    }
    
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    pub fn to_file(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new("127.0.0.1:8080".parse().unwrap(), 100)
    }
}