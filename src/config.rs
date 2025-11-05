use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use anyhow::Result;
use crate::stun_server::StunServerConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IceConfig {
    /// 是否启用ICE
    pub enable: bool,
    
    /// STUN服务器列表
    pub stun_servers: Vec<String>,
    
    /// 候选地址收集超时时间（毫秒）
    pub gathering_timeout: u64,
    
    /// 连接性检查超时时间（毫秒）
    pub connectivity_check_timeout: u64,
    
    /// 最大候选地址数量
    pub max_candidates: usize,
    
    /// STUN请求重试次数
    pub stun_retry_count: u32,
    
    /// STUN请求超时时间（毫秒）
    pub stun_timeout: u64,
    
    /// NAT端口预测配置
    pub port_prediction: PortPredictionConfig,
}

/// NAT端口预测配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PortPredictionConfig {
    /// 是否启用端口预测
    pub enable: bool,
    
    /// 最大预测端口数量
    pub max_predictions: usize,
    
    /// 最小样本数量
    pub min_samples: usize,
    
    /// 预测窗口大小
    pub prediction_window: u16,
    
    /// 是否启用IPv6预测
    pub enable_ipv6: bool,
    
    /// 端口范围限制 (最小端口, 最大端口)
    pub port_range: (u16, u16),
    
    /// 预测超时时间（毫秒）
    pub prediction_timeout_ms: u64,
    
    /// 是否启用端口验证
    pub enable_port_verification: bool,
    
    /// 端口验证超时时间（毫秒）
    pub verification_timeout_ms: u64,
    
    /// 是否启用NAT类型特定优化
    pub enable_nat_type_optimization: bool,
}

impl Default for PortPredictionConfig {
    fn default() -> Self {
        Self {
            enable: true,
            max_predictions: 10,
            min_samples: 3,
            prediction_window: 100,
            enable_ipv6: true,
            port_range: (1024, 65535),
            prediction_timeout_ms: 5000,
            enable_port_verification: true,
            verification_timeout_ms: 1000,
            enable_nat_type_optimization: true,
        }
    }
}

/// NAT类型检测配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NatDetectionConfig {
    /// 是否启用NAT类型检测
    pub enable: bool,
    
    /// STUN服务器列表用于NAT检测
    pub stun_servers: Vec<String>,
    
    /// 检测超时时间（毫秒）
    pub detection_timeout: u64,
    
    /// 检测重试次数
    pub retry_count: u32,
    
    /// 是否启用详细日志
    pub verbose_logging: bool,
}

impl Default for NatDetectionConfig {
    fn default() -> Self {
        Self {
            enable: true,
            stun_servers: vec![
                "stun.l.google.com:19302".to_string(),
                "stun1.l.google.com:19302".to_string(),
                "stun2.l.google.com:19302".to_string(),
            ],
            detection_timeout: 5000,
            retry_count: 3,
            verbose_logging: false,
        }
    }
}

impl Default for IceConfig {
    fn default() -> Self {
        Self {
            enable: true,
            stun_servers: vec![
                "stun.l.google.com:19302".to_string(),
                "stun1.l.google.com:19302".to_string(),
            ],
            gathering_timeout: 5000,
            connectivity_check_timeout: 30000,
            max_candidates: 10,
            stun_retry_count: 3,
            stun_timeout: 5000,
            port_prediction: PortPredictionConfig::default(),
        }
    }
}

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

    /// 节点列表广播去抖时间（毫秒），用于合并短时间内的拓扑变化
    pub peerlist_broadcast_debounce_ms: u64,

    /// ICE配置
    pub ice: IceConfig,
    
    /// STUN服务器配置
    pub stun_server: StunServerConfig,

    /// 是否允许为全对称NAT客户端转发流量
    pub allow_symmetric_nat_relay: bool,

    /// NAT类型检测配置
    pub nat_detection: NatDetectionConfig,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }
    
    #[allow(dead_code)]
    pub fn to_file(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_address: "127.0.0.1:8080".parse().unwrap(),
            max_connections: 100,
            heartbeat_interval: 30,
            connection_timeout: 60,
            discovery_port_range: (8081, 8090),
            enable_discovery: true,
            network_id: "p2p_default".to_string(),
            peerlist_broadcast_debounce_ms: 300,
            ice: IceConfig::default(),
            stun_server: StunServerConfig::default(),
            allow_symmetric_nat_relay: false,  // 默认不允许为全对称NAT转发流量
            nat_detection: NatDetectionConfig::default(),
        }
    }
}