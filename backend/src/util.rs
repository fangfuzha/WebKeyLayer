//! 工具函数模块

/// 获取本机局域网 IP
pub fn get_local_ip() -> Option<String> {
    // TODO: 使用 std::net 或第三方库获取本机 IP
    None
}

/// 验证端口是否可用
pub fn is_port_available(port: u16) -> bool {
    // TODO: 尝试绑定端口以检查其可用性
    true
}
