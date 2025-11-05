use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    /// 握手请求
    HandshakeRequest,
    /// 握手响应
    HandshakeResponse,
    /// 心跳包
    Ping,
    /// 心跳响应
    Pong,
    /// 节点发现请求
    DiscoveryRequest,
    /// 节点发现响应
    DiscoveryResponse,
    /// 请求节点列表
    ListNodesRequest,
    /// 响应节点列表
    ListNodesResponse,
    /// 数据传输
    Data,
    /// 错误消息
    Error,
    /// 断开连接
    Disconnect,
    /// 消息确认（UDP可靠性）
    Ack,
    /// 重传请求
    Retransmit,
    /// P2P 直连指令（NAT 打洞）
    P2PConnect,
    /// 流量转发请求（用于全对称NAT）
    RelayRequest,
    /// 流量转发响应
    RelayResponse,
    /// 转发的数据包
    RelayData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub message_type: MessageType,
    pub timestamp: u64,
    pub payload: serde_json::Value,
    /// 发送者地址（UDP需要）
    pub sender_addr: Option<SocketAddr>,
    /// 序列号（用于UDP重传和去重）
    pub sequence_number: Option<u32>,
    /// 是否需要确认
    pub requires_ack: bool,
    /// 确认的消息ID（用于Ack消息）
    pub ack_for: Option<Uuid>,
}

impl Message {
    pub fn new(message_type: MessageType, payload: serde_json::Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            message_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload,
            sender_addr: None,
            sequence_number: None,
            requires_ack: false,
            ack_for: None,
        }
    }
    
    /// 创建需要确认的消息
    pub fn new_with_ack(message_type: MessageType, payload: serde_json::Value, sender_addr: SocketAddr, sequence_number: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            message_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload,
            sender_addr: Some(sender_addr),
            sequence_number: Some(sequence_number),
            requires_ack: true,
            ack_for: None,
        }
    }
    
    /// 创建确认消息
    pub fn ack(original_message_id: Uuid, sender_addr: SocketAddr) -> Self {
        Self {
            id: Uuid::new_v4(),
            message_type: MessageType::Ack,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            payload: serde_json::Value::Null,
            sender_addr: Some(sender_addr),
            sequence_number: None,
            requires_ack: false,
            ack_for: Some(original_message_id),
        }
    }
    
    #[allow(dead_code)]
    pub fn handshake_request(node_info: NodeInfo) -> Self {
        let payload = serde_json::to_value(node_info).unwrap();
        Self::new(MessageType::HandshakeRequest, payload)
    }
    
    #[allow(dead_code)]
    pub fn handshake_response(node_info: NodeInfo, success: bool) -> Self {
        let response = HandshakeResponse {
            node_info,
            success,
            error_message: None,
            public_addr: None,
        };
        let payload = serde_json::to_value(response).unwrap();
        Self::new(MessageType::HandshakeResponse, payload)
    }

    /// 创建包含公网地址的握手响应
    pub fn handshake_response_with_public_addr(node_info: NodeInfo, success: bool, public_addr: SocketAddr) -> Self {
        let response = HandshakeResponse {
            node_info,
            success,
            error_message: None,
            public_addr: Some(public_addr),
        };
        let payload = serde_json::to_value(response).unwrap();
        Self::new(MessageType::HandshakeResponse, payload)
    }
    
    pub fn ping() -> Self {
        Self::new(MessageType::Ping, serde_json::Value::Null)
    }
    
    pub fn pong() -> Self {
        Self::new(MessageType::Pong, serde_json::Value::Null)
    }
    
    #[allow(dead_code)]
    pub fn discovery_request() -> Self {
        Self::new(MessageType::DiscoveryRequest, serde_json::Value::Null)
    }
    
    pub fn discovery_response(peers: Vec<PeerInfo>) -> Self {
        let payload = serde_json::to_value(peers).unwrap();
        Self::new(MessageType::DiscoveryResponse, payload)
    }
    
    pub fn data(data: serde_json::Value) -> Self {
        Self::new(MessageType::Data, data)
    }
    
    pub fn error(error_message: String) -> Self {
        let payload = serde_json::json!({ "error": error_message });
        Self::new(MessageType::Error, payload)
    }
    
    pub fn disconnect(reason: String) -> Self {
        let payload = serde_json::json!({ "reason": reason });
        Self::new(MessageType::Disconnect, payload)
    }

    #[allow(dead_code)]
    pub fn list_nodes_request() -> Self {
        Self::new(MessageType::ListNodesRequest, serde_json::Value::Null)
    }

    pub fn list_nodes_response(nodes: Vec<NodeInfo>) -> Self {
        let response = ListNodesResponse { nodes };
        let payload = serde_json::to_value(response).unwrap();
        Self::new(MessageType::ListNodesResponse, payload)
    }

    /// 发起 P2P 直连请求（由服务器协调打洞）
    #[allow(dead_code)]
    pub fn initiate_p2p(peer_id: Uuid) -> Self {
        let payload = serde_json::json!({ "peer_id": peer_id.to_string() });
        Self::new(MessageType::P2PConnect, payload)
    }

    /// 发起 P2P 直连请求，包含端口预测信息
    #[allow(dead_code)]
    pub fn initiate_p2p_with_prediction(
        peer_id: Uuid, 
        nat_type: Option<String>,
        predicted_ports: Option<Vec<u16>>,
        public_addr: Option<SocketAddr>
    ) -> Self {
        let mut payload = serde_json::json!({ "peer_id": peer_id.to_string() });
        
        if let Some(nat_type) = nat_type {
            payload["nat_type"] = serde_json::to_value(nat_type).unwrap_or(serde_json::Value::Null);
        }
        
        if let Some(ports) = predicted_ports {
            payload["predicted_ports"] = serde_json::to_value(ports).unwrap_or(serde_json::Value::Null);
        }
        
        if let Some(addr) = public_addr {
            payload["public_addr"] = serde_json::Value::String(addr.to_string());
        }
        
        Self::new(MessageType::P2PConnect, payload)
    }

    /// 创建流量转发请求
    #[allow(dead_code)]
    pub fn relay_request(target_peer_id: Uuid, data: Vec<u8>) -> Self {
        let mut payload = serde_json::Map::new();
        payload.insert("target_peer_id".to_string(), serde_json::Value::String(target_peer_id.to_string()));
        payload.insert("data".to_string(), serde_json::Value::Array(data.into_iter().map(|b| serde_json::Value::Number(serde_json::Number::from(b))).collect()));
        
        Self::new(MessageType::RelayRequest, serde_json::Value::Object(payload))
    }

    /// 创建流量转发响应
    pub fn relay_response(success: bool, error_message: Option<String>) -> Self {
        let mut payload = serde_json::Map::new();
        payload.insert("success".to_string(), serde_json::Value::Bool(success));
        if let Some(error) = error_message {
            payload.insert("error_message".to_string(), serde_json::Value::String(error));
        }
        
        Self::new(MessageType::RelayResponse, serde_json::Value::Object(payload))
    }

    /// 创建转发的数据包
    pub fn relay_data(from_peer_id: Uuid, data: Vec<u8>) -> Self {
        let mut payload = serde_json::Map::new();
        payload.insert("from_peer_id".to_string(), serde_json::Value::String(from_peer_id.to_string()));
        payload.insert("data".to_string(), serde_json::Value::Array(data.into_iter().map(|b| serde_json::Value::Number(serde_json::Number::from(b))).collect()));
        
        Self::new(MessageType::RelayData, serde_json::Value::Object(payload))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: Uuid,
    pub name: String,
    pub version: String,
    pub listen_addr: SocketAddr,
    pub capabilities: Vec<String>,
    pub metadata: HashMap<String, String>,
    pub network_id: String, // 新增 network_id 字段
}

impl NodeInfo {
    pub fn new(name: String, listen_addr: SocketAddr, network_id: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            version: env!("CARGO_PKG_VERSION").to_string(),
            listen_addr,
            capabilities: vec![
                "handshake".to_string(),
                "discovery".to_string(),
                "data_transfer".to_string(),
            ],
            metadata: HashMap::new(),
            network_id,
        }
    }
    
    #[allow(dead_code)]
    pub fn add_capability(&mut self, capability: String) {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
    }
    
    #[allow(dead_code)]
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub node_info: NodeInfo,
    pub success: bool,
    pub error_message: Option<String>,
    /// 客户端的公网地址（服务器看到的地址）
    pub public_addr: Option<SocketAddr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListNodesResponse {
    pub nodes: Vec<NodeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub id: Uuid,
    pub addr: SocketAddr,
    pub last_seen: u64,
    pub capabilities: Vec<String>,
}

impl PeerInfo {
    pub fn new(id: Uuid, addr: SocketAddr, capabilities: Vec<String>) -> Self {
        Self {
            id,
            addr,
            last_seen: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            capabilities,
        }
    }
    
    #[allow(dead_code)]
    pub fn update_last_seen(&mut self) {
        self.last_seen = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RelayRequest {
    pub target_peer_id: Uuid,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RelayResponse {
    pub success: bool,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RelayData {
    pub from_peer_id: Uuid,
    pub data: Vec<u8>,
}

/// 握手协议处理器
pub struct HandshakeProtocol;

impl HandshakeProtocol {
    /// 验证握手请求
    pub fn validate_handshake_request(message: &Message) -> Result<NodeInfo, String> {
        if message.message_type != MessageType::HandshakeRequest {
            return Err("不是握手请求消息".to_string());
        }
        
        let node_info: NodeInfo = serde_json::from_value(message.payload.clone())
            .map_err(|e| format!("解析节点信息失败: {}", e))?;
        
        // 验证节点信息
        if node_info.name.is_empty() {
            return Err("节点名称不能为空".to_string());
        }
        
        if node_info.version.is_empty() {
            return Err("节点版本不能为空".to_string());
        }
        
        Ok(node_info)
    }
    
    /// 验证握手响应
    pub fn validate_handshake_response(message: &Message) -> Result<HandshakeResponse, String> {
        if message.message_type != MessageType::HandshakeResponse {
            return Err("不是握手响应消息".to_string());
        }
        
        let response: HandshakeResponse = serde_json::from_value(message.payload.clone())
            .map_err(|e| format!("解析握手响应失败: {}", e))?;
        
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_creation() {
        let node_info = NodeInfo::new(
            "test_node".to_string(),
            "127.0.0.1:8080".parse().unwrap(),
            "testnet".to_string(),
        );
        let message = Message::handshake_request(node_info);
        
        assert_eq!(message.message_type, MessageType::HandshakeRequest);
        assert!(!message.id.is_nil());
    }
    
    #[test]
    fn test_handshake_validation() {
        let node_info = NodeInfo::new(
            "test_node".to_string(),
            "127.0.0.1:8080".parse().unwrap(),
            "testnet".to_string(),
        );
        let message = Message::handshake_request(node_info.clone());
        
        let result = HandshakeProtocol::validate_handshake_request(&message);
        assert!(result.is_ok());
        
        let validated_info = result.unwrap();
        assert_eq!(validated_info.name, node_info.name);
    }
}