use std::net::{SocketAddr, IpAddr, Ipv4Addr};
use anyhow::Result;
use rand::Rng;

/// STUN消息类型常量
pub const STUN_BINDING_REQUEST: u16 = 0x0001;
pub const STUN_BINDING_RESPONSE: u16 = 0x0101;
pub const STUN_BINDING_ERROR_RESPONSE: u16 = 0x0111;

/// STUN属性类型常量
pub const STUN_ATTR_MAPPED_ADDRESS: u16 = 0x0001;
pub const STUN_ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
pub const STUN_ATTR_SOFTWARE: u16 = 0x8022;
pub const STUN_ATTR_ERROR_CODE: u16 = 0x0009;

/// STUN魔法Cookie
pub const STUN_MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN消息结构
#[derive(Debug, Clone)]
pub struct StunMessage {
    pub message_type: u16,
    pub length: u16,
    pub magic_cookie: u32,
    pub transaction_id: [u8; 12],
    pub attributes: Vec<StunAttribute>,
}

/// STUN属性结构
#[derive(Debug, Clone)]
pub struct StunAttribute {
    pub attr_type: u16,
    pub length: u16,
    pub value: Vec<u8>,
}

impl StunMessage {
    /// 创建STUN Binding Request
    pub fn new_binding_request() -> Self {
        let mut rng = rand::thread_rng();
        let mut transaction_id = [0u8; 12];
        rng.fill(&mut transaction_id);

        Self {
            message_type: STUN_BINDING_REQUEST,
            length: 0,
            magic_cookie: STUN_MAGIC_COOKIE,
            transaction_id,
            attributes: Vec::new(),
        }
    }

    /// 创建STUN Binding Response
    pub fn new_binding_response(transaction_id: [u8; 12]) -> Self {
        Self {
            message_type: STUN_BINDING_RESPONSE,
            length: 0,
            magic_cookie: STUN_MAGIC_COOKIE,
            transaction_id,
            attributes: Vec::new(),
        }
    }

    /// 创建STUN Error Response
    pub fn new_error_response(transaction_id: [u8; 12], error_code: u16, reason: &str) -> Self {
        let mut message = Self {
            message_type: STUN_BINDING_ERROR_RESPONSE,
            length: 0,
            magic_cookie: STUN_MAGIC_COOKIE,
            transaction_id,
            attributes: Vec::new(),
        };

        // 添加错误码属性
        let mut error_value = Vec::new();
        error_value.extend_from_slice(&[0u8, 0u8]); // 保留字段
        error_value.push((error_code / 100) as u8); // 错误类别
        error_value.push((error_code % 100) as u8); // 错误号
        error_value.extend_from_slice(reason.as_bytes());

        message.add_attribute(StunAttribute {
            attr_type: STUN_ATTR_ERROR_CODE,
            length: error_value.len() as u16,
            value: error_value,
        });

        message
    }

    /// 添加属性
    pub fn add_attribute(&mut self, attribute: StunAttribute) {
        self.attributes.push(attribute);
        self.update_length();
    }

    /// 更新消息长度
    fn update_length(&mut self) {
        let mut length = 0;
        for attr in &self.attributes {
            length += 4; // 属性头部
            length += attr.value.len();
            // 4字节对齐填充
            let padding = (4 - (attr.value.len() % 4)) % 4;
            length += padding;
        }
        self.length = length as u16;
    }

    /// 序列化为字节数组
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        
        // STUN头部 (20字节)
        bytes.extend_from_slice(&self.message_type.to_be_bytes());
        bytes.extend_from_slice(&self.length.to_be_bytes());
        bytes.extend_from_slice(&self.magic_cookie.to_be_bytes());
        bytes.extend_from_slice(&self.transaction_id);
        
        // 属性
        for attr in &self.attributes {
            bytes.extend_from_slice(&attr.attr_type.to_be_bytes());
            bytes.extend_from_slice(&attr.length.to_be_bytes());
            bytes.extend_from_slice(&attr.value);
            
            // 4字节对齐填充
            let padding = (4 - (attr.value.len() % 4)) % 4;
            bytes.extend_from_slice(&vec![0u8; padding]);
        }
        
        bytes
    }

    /// 从字节数组解析
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 20 {
            return Err(anyhow::anyhow!("STUN消息太短"));
        }

        let message_type = u16::from_be_bytes([data[0], data[1]]);
        let length = u16::from_be_bytes([data[2], data[3]]);
        let magic_cookie = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        
        if magic_cookie != STUN_MAGIC_COOKIE {
            return Err(anyhow::anyhow!("无效的STUN魔法Cookie"));
        }

        let mut transaction_id = [0u8; 12];
        transaction_id.copy_from_slice(&data[8..20]);

        let mut attributes = Vec::new();
        let mut offset = 20;

        while offset < data.len() {
            if offset + 4 > data.len() {
                break;
            }

            let attr_type = u16::from_be_bytes([data[offset], data[offset + 1]]);
            let attr_length = u16::from_be_bytes([data[offset + 2], data[offset + 3]]);
            offset += 4;

            if offset + attr_length as usize > data.len() {
                break;
            }

            let value = data[offset..offset + attr_length as usize].to_vec();
            offset += attr_length as usize;

            // 跳过填充字节
            let padding = (4 - (attr_length as usize % 4)) % 4;
            offset += padding;

            attributes.push(StunAttribute {
                attr_type,
                length: attr_length,
                value,
            });
        }

        Ok(Self {
            message_type,
            length,
            magic_cookie,
            transaction_id,
            attributes,
        })
    }

    /// 提取映射地址
    pub fn extract_mapped_address(&self) -> Option<SocketAddr> {
        for attr in &self.attributes {
            if attr.attr_type == STUN_ATTR_MAPPED_ADDRESS || attr.attr_type == STUN_ATTR_XOR_MAPPED_ADDRESS {
                return self.parse_address_attribute(&attr.value, attr.attr_type == STUN_ATTR_XOR_MAPPED_ADDRESS);
            }
        }
        None
    }

    /// 解析地址属性
    fn parse_address_attribute(&self, data: &[u8], is_xor: bool) -> Option<SocketAddr> {
        if data.len() < 8 {
            return None;
        }

        let family = u16::from_be_bytes([data[0], data[1]]);
        if family != 0x0001 { // IPv4
            return None;
        }

        let mut port = u16::from_be_bytes([data[2], data[3]]);
        let mut ip_bytes = [data[4], data[5], data[6], data[7]];

        if is_xor {
            // XOR解码
            port ^= (STUN_MAGIC_COOKIE >> 16) as u16;
            let magic_bytes = STUN_MAGIC_COOKIE.to_be_bytes();
            for i in 0..4 {
                ip_bytes[i] ^= magic_bytes[i];
            }
        }

        let ip = Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]);
        Some(SocketAddr::new(IpAddr::V4(ip), port))
    }
}

/// 检查数据包是否为STUN消息
pub fn is_stun_packet(data: &[u8]) -> bool {
    if data.len() < 20 {
        return false;
    }

    // 检查魔法Cookie
    let magic_cookie = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    magic_cookie == STUN_MAGIC_COOKIE
}

/// 从STUN数据包中提取事务ID
#[allow(dead_code)]
pub fn extract_transaction_id(data: &[u8]) -> Option<[u8; 12]> {
    if data.len() < 20 {
        return None;
    }

    let mut transaction_id = [0u8; 12];
    transaction_id.copy_from_slice(&data[8..20]);
    Some(transaction_id)
}

/// 创建映射地址属性
#[allow(dead_code)]
pub fn create_mapped_address_attribute(addr: SocketAddr, use_xor: bool) -> StunAttribute {
    let mut value = Vec::new();
    
    // 地址族 (IPv4 = 0x0001)
    value.extend_from_slice(&0x0001u16.to_be_bytes());
    
    let (ip_bytes, port) = match addr {
        SocketAddr::V4(addr_v4) => {
            let ip = addr_v4.ip().octets();
            let port = addr_v4.port();
            (ip, port)
        }
        SocketAddr::V6(_) => {
            // 暂不支持IPv6
            return StunAttribute {
                attr_type: if use_xor { STUN_ATTR_XOR_MAPPED_ADDRESS } else { STUN_ATTR_MAPPED_ADDRESS },
                length: 0,
                value: Vec::new(),
            };
        }
    };

    if use_xor {
        // XOR编码
        let xor_port = port ^ (STUN_MAGIC_COOKIE >> 16) as u16;
        value.extend_from_slice(&xor_port.to_be_bytes());
        
        let magic_bytes = STUN_MAGIC_COOKIE.to_be_bytes();
        for i in 0..4 {
            value.push(ip_bytes[i] ^ magic_bytes[i]);
        }
    } else {
        // 普通编码
        value.extend_from_slice(&port.to_be_bytes());
        value.extend_from_slice(&ip_bytes);
    }

    StunAttribute {
        attr_type: if use_xor { STUN_ATTR_XOR_MAPPED_ADDRESS } else { STUN_ATTR_MAPPED_ADDRESS },
        length: value.len() as u16,
        value,
    }
}

/// 创建软件属性
#[allow(dead_code)]
pub fn create_software_attribute(software: &str) -> StunAttribute {
    StunAttribute {
        attr_type: STUN_ATTR_SOFTWARE,
        length: software.len() as u16,
        value: software.as_bytes().to_vec(),
    }
}