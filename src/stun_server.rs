use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use anyhow::{Result, Context};
use log::{info, debug, warn, error};
use serde::{Serialize, Deserialize};

// 使用共享的STUN协议模块
use crate::stun_protocol::{
    StunMessage, 
    STUN_BINDING_REQUEST, 
    create_mapped_address_attribute,
    create_software_attribute,
};

/// STUN错误码常量
const STUN_ERROR_BAD_REQUEST: u16 = 400;
#[allow(dead_code)]
const STUN_ERROR_SERVER_ERROR: u16 = 500;

/// STUN服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StunServerConfig {
    /// 是否启用STUN服务器
    pub enable: bool,
    /// STUN服务器监听端口
    pub port: u16,
    /// 软件标识字符串
    pub software: String,
    /// 是否启用详细日志
    pub verbose_logging: bool,
    /// 最大并发连接数
    pub max_concurrent_requests: usize,
}

impl Default for StunServerConfig {
    fn default() -> Self {
        Self {
            enable: false,  // 默认关闭STUN服务器
            port: 3478,
            software: "P2P-Handshake-Server/1.0".to_string(),
            verbose_logging: false,
            max_concurrent_requests: 1000,
        }
    }
}

/// STUN服务器实现
pub struct StunServer {
    config: StunServerConfig,
    socket: Arc<UdpSocket>,
    local_addr: SocketAddr,
}

impl StunServer {
    /// 创建新的STUN服务器实例
    pub async fn new(config: StunServerConfig, bind_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind(bind_addr).await
            .context("绑定STUN服务器套接字失败")?;
        
        let local_addr = socket.local_addr()
            .context("获取STUN服务器本地地址失败")?;
        
        info!("STUN服务器启动成功，监听地址: {}", local_addr);
        
        Ok(Self {
            config,
            socket: Arc::new(socket),
            local_addr,
        })
    }

    /// 获取本地监听地址
    #[allow(dead_code)]
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// 启动STUN服务器
    pub async fn run(&self) -> Result<()> {
        info!("STUN服务器开始运行，监听端口: {}", self.local_addr.port());
        
        let mut buffer = vec![0u8; 1500]; // MTU大小的缓冲区
        
        loop {
            match self.socket.recv_from(&mut buffer).await {
                Ok((len, client_addr)) => {
                    if self.config.verbose_logging {
                        debug!("收到来自 {} 的STUN请求，长度: {} 字节", client_addr, len);
                    }
                    
                    // 处理STUN请求
                    if let Err(e) = self.handle_stun_request(&buffer[..len], client_addr).await {
                        warn!("处理来自 {} 的STUN请求失败: {}", client_addr, e);
                    }
                }
                Err(e) => {
                    error!("接收STUN数据包失败: {}", e);
                    // 继续运行，不因单个错误而停止服务
                }
            }
        }
    }

    /// 处理STUN请求
    async fn handle_stun_request(&self, data: &[u8], client_addr: SocketAddr) -> Result<()> {
        // 解析STUN消息
        let request = match StunMessage::from_bytes(data) {
            Ok(msg) => msg,
            Err(e) => {
                debug!("解析STUN消息失败: {}", e);
                // 发送错误响应
                self.send_error_response(client_addr, [0; 12], STUN_ERROR_BAD_REQUEST, "Bad Request").await?;
                return Ok(());
            }
        };

        if self.config.verbose_logging {
            debug!("解析STUN消息成功: 类型={:04x}, 事务ID={:?}", 
                   request.message_type, request.transaction_id);
        }

        // 处理不同类型的STUN请求
        match request.message_type {
            STUN_BINDING_REQUEST => {
                self.handle_binding_request(&request, client_addr).await?;
            }
            _ => {
                debug!("不支持的STUN消息类型: {:04x}", request.message_type);
                self.send_error_response(
                    client_addr, 
                    request.transaction_id, 
                    STUN_ERROR_BAD_REQUEST, 
                    "Unsupported Message Type"
                ).await?;
            }
        }

        Ok(())
    }

    /// 处理STUN绑定请求
    async fn handle_binding_request(&self, request: &StunMessage, client_addr: SocketAddr) -> Result<()> {
        if self.config.verbose_logging {
            debug!("处理来自 {} 的STUN绑定请求", client_addr);
        }

        // 创建绑定响应
        let response = self.create_binding_response(request, client_addr)?;
        let response_bytes = response.to_bytes();

        // 发送响应
        match self.socket.send_to(&response_bytes, client_addr).await {
            Ok(sent) => {
                if self.config.verbose_logging {
                    debug!("向 {} 发送STUN绑定响应成功，发送 {} 字节", client_addr, sent);
                }
            }
            Err(e) => {
                warn!("向 {} 发送STUN绑定响应失败: {}", client_addr, e);
                return Err(e.into());
            }
        }

        Ok(())
    }

    /// 创建STUN绑定响应
    fn create_binding_response(&self, request: &StunMessage, client_addr: SocketAddr) -> Result<StunMessage> {
        let mut response = StunMessage::new_binding_response(request.transaction_id);

        // 添加XOR映射地址属性（RFC 5389推荐）
        let xor_mapped_attr = create_mapped_address_attribute(client_addr, true);
        response.add_attribute(xor_mapped_attr);

        // 添加映射地址属性（向后兼容）
        let mapped_attr = create_mapped_address_attribute(client_addr, false);
        response.add_attribute(mapped_attr);

        // 添加软件属性
        let software_attr = create_software_attribute(&self.config.software);
        response.add_attribute(software_attr);

        Ok(response)
    }



    /// 发送错误响应
    async fn send_error_response(
        &self, 
        client_addr: SocketAddr, 
        transaction_id: [u8; 12], 
        error_code: u16, 
        reason_phrase: &str
    ) -> Result<()> {
        let mut response = StunMessage::new_error_response(transaction_id, error_code, reason_phrase);

        // 添加软件属性
        let software_attr = create_software_attribute(&self.config.software);
        response.add_attribute(software_attr);

        let response_bytes = response.to_bytes();
        
        match self.socket.send_to(&response_bytes, client_addr).await {
            Ok(_) => {
                debug!("向 {} 发送STUN错误响应: {} {}", client_addr, error_code, reason_phrase);
            }
            Err(e) => {
                warn!("向 {} 发送STUN错误响应失败: {}", client_addr, e);
                return Err(e.into());
            }
        }

        Ok(())
    }



    /// 获取服务器统计信息
    #[allow(dead_code)]
    pub async fn get_stats(&self) -> StunServerStats {
        StunServerStats {
            local_addr: self.local_addr,
            is_running: true,
            config: self.config.clone(),
        }
    }
}

/// STUN服务器统计信息
#[derive(Debug, Clone)]
pub struct StunServerStats {
    #[allow(dead_code)]
    pub local_addr: SocketAddr,
    #[allow(dead_code)]
    pub is_running: bool,
    #[allow(dead_code)]
    pub config: StunServerConfig,
}