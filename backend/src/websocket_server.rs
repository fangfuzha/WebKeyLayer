//! WebSocket 事件分发层
//!
//! 负责 WebSocket 服务器的实现，支持多客户端连接和按键事件广播。

use crate::mouse_hook::{MouseDirection, MouseMotionEvent};
use crate::Result;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

/// WebSocket 服务器配置
#[derive(Debug, Clone)]
pub struct WebSocketConfig {
    /// 绑定地址（默认 0.0.0.0）
    pub bind_address: String,
    /// 监听端口（默认 8080）
    pub port: u16,
}

impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            bind_address: "0.0.0.0".to_string(),
            port: 8080,
        }
    }
}

/// WebSocket 服务器
pub struct WebSocketServer {
    config: WebSocketConfig,
}

impl WebSocketServer {
    /// 创建 WebSocket 服务器实例。
    ///
    /// 参数:
    /// - `config`: 服务绑定配置
    ///
    /// 返回:
    /// - 新的服务器实例
    pub fn new(config: WebSocketConfig) -> Self {
        Self { config }
    }

    /// 启动 WebSocket 服务
    pub async fn start(&self) -> Result<()> {
        // TODO: 实现 Tokio 异步 WebSocket 服务
        // 1. 创建监听地址：{bind_address}:{port}
        // 2. 为每个连接创建独立处理任务
        // 3. 维护连接列表用于广播
        let endpoint = format!("{}:{}", self.config.bind_address, self.config.port);
        debug!(endpoint, "websocket server start requested");
        Ok(())
    }

    /// 广播按键事件到所有客户端
    pub async fn broadcast_key_event(&self, keycode: u16, pressed: bool) -> Result<()> {
        let event_type = if pressed { "key_pressed" } else { "key_released" };
        let payload = json!({
            "keycode": keycode,
            "pressed": pressed,
        });
        self.broadcast_json(event_type, payload).await
    }

    /// 广播鼠标按键事件到所有客户端
    pub async fn broadcast_mouse_button(
        &self,
        button: u8,
        pressed: bool,
        x: i32,
        y: i32,
    ) -> Result<()> {
        let event_type = if pressed {
            "mouse_button_pressed"
        } else {
            "mouse_button_released"
        };
        let payload = json!({
            "button": button,
            "pressed": pressed,
            "x": x,
            "y": y,
        });
        self.broadcast_json(event_type, payload).await
    }

    /// 广播鼠标移动事件到所有客户端
    ///
    /// 现在协议采用相对位移 (dx, dy) + 离散方向，且仅在方向变化时发送。
    pub async fn broadcast_mouse_move(&self, dx: i32, dy: i32, direction: MouseDirection) -> Result<()> {
        let payload = json!({
            "dx": dx,
            "dy": dy,
            "direction": direction.as_str(),
        });
        self.broadcast_json("mouse_move_direction_changed", payload).await
    }

    /// 广播鼠标静止事件到所有客户端
    pub async fn broadcast_mouse_idle(&self) -> Result<()> {
        let payload = json!({
            "state": "idle",
        });
        self.broadcast_json("mouse_idle", payload).await
    }

    /// 广播鼠标运动采样事件到所有客户端。
    ///
    /// 参数:
    /// - `event`: 鼠标采样状态机输出事件
    pub async fn broadcast_mouse_motion_event(&self, event: MouseMotionEvent) -> Result<()> {
        match event {
            MouseMotionEvent::DirectionChanged { dx, dy, direction } => {
                self.broadcast_mouse_move(dx, dy, direction).await
            }
            MouseMotionEvent::Idle => self.broadcast_mouse_idle().await,
        }
    }

    /// 广播鼠标滚轮事件到所有客户端
    pub async fn broadcast_mouse_wheel(&self, delta: i32, x: i32, y: i32) -> Result<()> {
        let payload = json!({
            "delta": delta,
            "x": x,
            "y": y,
        });
        self.broadcast_json("mouse_wheel", payload).await
    }

    /// 将事件按统一信封序列化并执行广播。
    ///
    /// 参数:
    /// - `event_type`: 协议事件类型
    /// - `payload`: 事件负载
    async fn broadcast_json(&self, event_type: &str, payload: Value) -> Result<()> {
        let message = Envelope::new(event_type, payload);
        let serialized = serde_json::to_string(&message)?;
        // TODO: 接入真实连接管理并发送到所有客户端
        debug!(%serialized, "websocket broadcast queued");
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct Envelope {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    timestamp: u64,
    payload: Value,
}

impl Envelope {
    /// 创建统一事件信封。
    ///
    /// 参数:
    /// - `event_type`: 协议事件类型
    /// - `payload`: 协议负载
    ///
    /// 返回:
    /// - 可序列化的统一信封
    fn new(event_type: &str, payload: Value) -> Self {
        let timestamp = current_timestamp_millis();
        Self {
            id: format!("{}_{}", event_type, timestamp),
            kind: event_type.to_string(),
            timestamp,
            payload,
        }
    }
}

/// 获取当前 Unix 毫秒时间戳。
///
/// 返回:
/// - 毫秒级时间戳；在系统时间异常时返回 0
fn current_timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
