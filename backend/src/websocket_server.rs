//! WebSocket 事件分发层
//!
//! 负责 WebSocket 服务器的实现，支持多客户端连接和按键事件广播。

use crate::mouse_hook::{MouseDirection, MouseMotionEvent};
use crate::Result;
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{json, Value};
use std::env;
use std::fmt::Display;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast::{self, Receiver, Sender};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, WebSocketStream};
use tracing::{debug, info, trace, warn};

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
#[derive(Clone)]
pub struct WebSocketServer {
    config: WebSocketConfig,
    outbound: Sender<String>,
    message_sequence: Arc<AtomicU64>,
    client_sequence: Arc<AtomicU64>,
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
        let (outbound, _) = broadcast::channel(1024);
        Self {
            config,
            outbound,
            message_sequence: Arc::new(AtomicU64::new(1)),
            client_sequence: Arc::new(AtomicU64::new(1)),
        }
    }

    /// 启动 WebSocket 服务
    pub async fn start(&self) -> Result<()> {
        let endpoint = format!("{}:{}", self.config.bind_address, self.config.port);
        let listener = TcpListener::bind(&endpoint).await?;
        let outbound = self.outbound.clone();
        let message_sequence = Arc::clone(&self.message_sequence);
        let client_sequence = Arc::clone(&self.client_sequence);
        let public_root = locate_public_root();

        tokio::spawn(async move {
            info!(%endpoint, "websocket server listening");
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        let outbound = outbound.clone();
                        let message_sequence = Arc::clone(&message_sequence);
                        let client_sequence = Arc::clone(&client_sequence);
                        let public_root = public_root.clone();
                        tokio::spawn(async move {
                            match is_websocket_request(&stream).await {
                                Ok(true) => {
                                    let client_id = format!(
                                        "client_{}",
                                        client_sequence.fetch_add(1, Ordering::Relaxed)
                                    );
                                    let receiver = outbound.subscribe();
                                    if let Err(error) = handle_connection(
                                        stream,
                                        peer_addr,
                                        receiver,
                                        outbound,
                                        message_sequence,
                                        client_id,
                                    )
                                    .await
                                    {
                                        warn!(%peer_addr, %error, "websocket client handler stopped");
                                    }
                                }
                                Ok(false) => {
                                    if let Err(error) =
                                        handle_http_connection(stream, peer_addr, public_root).await
                                    {
                                        warn!(%peer_addr, %error, "http client handler stopped");
                                    }
                                }
                                Err(error) => {
                                    warn!(%peer_addr, %error, "failed to inspect incoming connection");
                                }
                            }
                        });
                    }
                    Err(error) => {
                        warn!(%error, "failed to accept websocket tcp connection");
                    }
                }
            }
        });

        Ok(())
    }

    /// 返回当前广播接收端数量。
    ///
    /// 返回:
    /// - 已订阅广播通道的客户端数量
    pub fn connected_clients(&self) -> usize {
        self.outbound.receiver_count()
    }

    /// 广播按键事件到所有客户端
    pub async fn broadcast_key_event(&self, keycode: u16, pressed: bool) -> Result<()> {
        let event_type = if pressed {
            "key_pressed"
        } else {
            "key_released"
        };
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
    pub async fn broadcast_mouse_move(
        &self,
        dx: i32,
        dy: i32,
        direction: MouseDirection,
    ) -> Result<()> {
        let payload = json!({
            "dx": dx,
            "dy": dy,
            "direction": direction.as_str(),
        });
        self.broadcast_json("mouse_move_direction_changed", payload)
            .await
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
        let message = Envelope::new(self.next_message_id(event_type), event_type, payload);
        let serialized = serde_json::to_string(&message)?;

        match self.outbound.send(serialized) {
            Ok(receivers) => {
                trace!(event_type, receivers, "websocket broadcast sent");
            }
            Err(error) => {
                trace!(event_type, message = %error.0, "websocket broadcast skipped without clients");
            }
        }

        Ok(())
    }

    /// 为事件生成单调递增消息 ID。
    ///
    /// 参数:
    /// - `event_type`: 协议事件类型
    ///
    /// 返回:
    /// - 可用于协议信封的消息 ID
    fn next_message_id(&self, event_type: &str) -> String {
        next_message_id(event_type, &self.message_sequence)
    }
}

/// 判断连接是否为 WebSocket upgrade 请求。
///
/// 参数:
/// - `stream`: 待检查的 TCP 连接
///
/// 返回:
/// - `true` 表示这是 WebSocket 请求，`false` 表示普通 HTTP 请求
async fn is_websocket_request(stream: &TcpStream) -> Result<bool> {
    let mut buffer = [0_u8; 1024];
    let size = stream.peek(&mut buffer).await?;
    let headers = String::from_utf8_lossy(&buffer[..size]).to_ascii_lowercase();
    Ok(headers.contains("upgrade: websocket"))
}

/// 处理普通 HTTP 请求并返回推流页静态资源。
///
/// 参数:
/// - `stream`: 已建立的 TCP 连接
/// - `peer_addr`: 客户端地址
/// - `public_root`: 前端静态资源根目录
///
/// 返回:
/// - 静态资源成功写入或连接正常结束时返回 `Ok(())`
async fn handle_http_connection(
    mut stream: TcpStream,
    peer_addr: SocketAddr,
    public_root: PathBuf,
) -> Result<()> {
    let mut buffer = [0_u8; 8192];
    let size = stream.read(&mut buffer).await?;
    if size == 0 {
        return Ok(());
    }

    let request = String::from_utf8_lossy(&buffer[..size]);
    let request_path = request
        .lines()
        .next()
        .and_then(parse_http_request_path)
        .unwrap_or("/");

    let response = build_static_response(request_path, &public_root).await?;
    stream.write_all(&response).await?;
    stream.shutdown().await?;
    trace!(%peer_addr, request_path, "http static response sent");
    Ok(())
}

/// 从 HTTP 请求行解析路径。
///
/// 参数:
/// - `request_line`: HTTP 请求首行
///
/// 返回:
/// - 请求路径；无法解析时返回 `None`
fn parse_http_request_path(request_line: &str) -> Option<&str> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    let path = parts.next()?;
    if method == "GET" || method == "HEAD" {
        Some(path)
    } else {
        None
    }
}

/// 构造静态资源 HTTP 响应。
///
/// 参数:
/// - `request_path`: 请求路径
/// - `public_root`: 前端静态资源根目录
///
/// 返回:
/// - 完整 HTTP 响应字节
async fn build_static_response(request_path: &str, public_root: &Path) -> Result<Vec<u8>> {
    let path_without_query = request_path.split('?').next().unwrap_or(request_path);
    if path_without_query == "/" || path_without_query == "/public" {
        return Ok(http_redirect("/public/"));
    }

    let Some(file_path) = public_file_path(path_without_query, public_root) else {
        return Ok(http_response(
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"Not Found".to_vec(),
        ));
    };

    match tokio::fs::read(&file_path).await {
        Ok(body) => Ok(http_response("200 OK", content_type(&file_path), body)),
        Err(_) => Ok(http_response(
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"Not Found".to_vec(),
        )),
    }
}

/// 将 `/public/...` 请求路径映射到本地文件路径。
///
/// 参数:
/// - `request_path`: 请求路径
/// - `public_root`: 前端静态资源根目录
///
/// 返回:
/// - 安全的本地文件路径；非法路径返回 `None`
fn public_file_path(request_path: &str, public_root: &Path) -> Option<PathBuf> {
    let relative = match request_path {
        "/public/" => "index.html",
        path if path.starts_with("/public/") => path.trim_start_matches("/public/"),
        _ => return None,
    };

    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };

    let relative_path = Path::new(relative);
    let mut safe_path = PathBuf::new();
    for component in relative_path.components() {
        match component {
            Component::Normal(part) => safe_path.push(part),
            _ => return None,
        }
    }

    Some(public_root.join(safe_path))
}

/// 返回静态文件的 Content-Type。
///
/// 参数:
/// - `file_path`: 静态文件路径
///
/// 返回:
/// - MIME 类型
fn content_type(file_path: &Path) -> &'static str {
    match file_path
        .extension()
        .and_then(|extension| extension.to_str())
    {
        Some("html") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

/// 构造普通 HTTP 响应。
///
/// 参数:
/// - `status`: HTTP 状态行中的状态文本
/// - `content_type`: 响应 MIME 类型
/// - `body`: 响应体
///
/// 返回:
/// - 完整 HTTP 响应字节
fn http_response(status: &str, content_type: &str, body: Vec<u8>) -> Vec<u8> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    [headers.as_bytes(), &body].concat()
}

/// 构造 HTTP 重定向响应。
///
/// 参数:
/// - `location`: 目标路径
///
/// 返回:
/// - 完整 HTTP 响应字节
fn http_redirect(location: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    )
    .into_bytes()
}

/// 定位前端推流页静态资源目录。
///
/// 返回:
/// - `frontend/public` 目录路径；目录不存在时返回基于后端 manifest 的默认路径
fn locate_public_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let current_dir = env::current_dir().unwrap_or_else(|_| manifest_dir.clone());
    let candidates = [
        current_dir.join("frontend").join("public"),
        current_dir.join("..").join("frontend").join("public"),
        manifest_dir.join("..").join("frontend").join("public"),
    ];

    candidates
        .iter()
        .find(|candidate| candidate.exists())
        .cloned()
        .unwrap_or_else(|| manifest_dir.join("..").join("frontend").join("public"))
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
    /// - `id`: 消息 ID
    /// - `event_type`: 协议事件类型
    /// - `payload`: 协议负载
    ///
    /// 返回:
    /// - 可序列化的统一信封
    fn new(id: String, event_type: &str, payload: Value) -> Self {
        let timestamp = current_timestamp_millis();
        Self {
            id,
            kind: event_type.to_string(),
            timestamp,
            payload,
        }
    }
}

/// 处理单个 WebSocket 客户端连接。
///
/// 参数:
/// - `stream`: 已建立的 TCP 连接
/// - `peer_addr`: 客户端地址
/// - `receiver`: 广播消息接收器
/// - `outbound`: 广播发送器，用于统计连接数
/// - `message_sequence`: 全局消息序列
/// - `client_id`: 当前客户端 ID
///
/// 返回:
/// - 客户端正常或异常断开时返回结果
async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    mut receiver: Receiver<String>,
    outbound: Sender<String>,
    message_sequence: Arc<AtomicU64>,
    client_id: String,
) -> Result<()> {
    let websocket = accept_async(stream)
        .await
        .map_err(|error| websocket_error("handshake failed", error))?;
    let (mut write, mut read) = websocket.split();

    let established = Envelope::new(
        next_message_id("connection_established", &message_sequence),
        "connection_established",
        json!({
            "server_version": crate::VERSION,
            "protocol_version": "1.0",
            "client_id": client_id,
        }),
    );
    let established = serde_json::to_string(&established)?;
    write
        .send(Message::Text(established))
        .await
        .map_err(|error| websocket_error("failed to send connection established", error))?;

    info!(%peer_addr, "websocket client connected");

    loop {
        tokio::select! {
            broadcast = receiver.recv() => {
                match broadcast {
                    Ok(serialized) => {
                        write
                            .send(Message::Text(serialized))
                            .await
                            .map_err(|error| websocket_error("failed to send broadcast", error))?;
                    }
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(%peer_addr, skipped, "websocket client lagged behind broadcast channel");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            incoming = read.next() => {
                match incoming {
                    Some(Ok(message)) => {
                        let keep_open = handle_client_message(
                            message,
                            &mut write,
                            &outbound,
                            &message_sequence,
                        )
                        .await?;
                        if !keep_open {
                            break;
                        }
                    }
                    Some(Err(error)) => {
                        return Err(websocket_error("failed to read websocket message", error));
                    }
                    None => break,
                }
            }
        }
    }

    info!(%peer_addr, "websocket client disconnected");
    Ok(())
}

/// 处理来自客户端的控制消息。
///
/// 参数:
/// - `message`: 客户端消息
/// - `write`: 当前客户端写半部
/// - `outbound`: 广播发送器，用于生成心跳统计
/// - `message_sequence`: 全局消息序列
///
/// 返回:
/// - `true` 表示保持连接，`false` 表示关闭连接
async fn handle_client_message(
    message: Message,
    write: &mut SplitSink<WebSocketStream<TcpStream>, Message>,
    outbound: &Sender<String>,
    message_sequence: &AtomicU64,
) -> Result<bool> {
    match message {
        Message::Text(text) => {
            if is_heartbeat_message(&text) {
                let ack = Envelope::new(
                    next_message_id("heartbeat_ack", message_sequence),
                    "heartbeat_ack",
                    json!({
                        "server_time": current_timestamp_millis(),
                        "connected_clients": outbound.receiver_count(),
                    }),
                );
                let serialized = serde_json::to_string(&ack)?;
                write
                    .send(Message::Text(serialized))
                    .await
                    .map_err(|error| websocket_error("failed to send heartbeat ack", error))?;
            } else {
                debug!(%text, "websocket client text message ignored");
            }
            Ok(true)
        }
        Message::Binary(data) => {
            debug!(
                bytes = data.len(),
                "websocket client binary message ignored"
            );
            Ok(true)
        }
        Message::Ping(payload) => {
            write
                .send(Message::Pong(payload))
                .await
                .map_err(|error| websocket_error("failed to send pong", error))?;
            Ok(true)
        }
        Message::Pong(_) => Ok(true),
        Message::Close(frame) => {
            write
                .send(Message::Close(frame))
                .await
                .map_err(|error| websocket_error("failed to send close frame", error))?;
            Ok(false)
        }
        _ => Ok(true),
    }
}

/// 判断文本消息是否为协议心跳。
///
/// 参数:
/// - `text`: 客户端文本消息
///
/// 返回:
/// - `true` 表示消息类型为 `heartbeat`
fn is_heartbeat_message(text: &str) -> bool {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .is_some_and(|message_type| message_type == "heartbeat")
}

/// 生成协议消息 ID。
///
/// 参数:
/// - `event_type`: 协议事件类型
/// - `sequence`: 全局消息序列
///
/// 返回:
/// - 单调递增消息 ID
fn next_message_id(event_type: &str, sequence: &AtomicU64) -> String {
    let sequence = sequence.fetch_add(1, Ordering::Relaxed);
    format!("{}_{}", event_type, sequence)
}

/// 将 WebSocket 底层错误包装为统一错误类型。
///
/// 参数:
/// - `context`: 错误发生场景
/// - `error`: 底层错误
///
/// 返回:
/// - 统一错误类型
fn websocket_error(context: &str, error: impl Display) -> crate::Error {
    crate::Error::WebSocket(format!("{}: {}", context, error))
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
