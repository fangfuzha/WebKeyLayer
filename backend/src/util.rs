//! 工具函数模块

use std::net::{TcpListener, UdpSocket};

/// 获取本机局域网 IP
///
/// 返回:
/// - 能用于局域网访问的本机 IPv4 地址；无法判断时返回 `None`
pub fn get_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let address = socket.local_addr().ok()?;
    Some(address.ip().to_string())
}

/// 验证端口是否可用
///
/// 参数:
/// - `port`: 待检测端口
///
/// 返回:
/// - `true` 表示当前进程可以绑定该端口
pub fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}
