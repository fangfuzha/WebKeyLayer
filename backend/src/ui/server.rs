//! 后端管理页面 API 服务

use crate::config::{Config, ConfigLoader};
use crate::keyboard_hook::KeyboardHook;
use crate::mouse_hook::MouseHook;
use crate::preset::{ImportMode, Preset, PresetImporter};
use crate::util::get_local_ip;
use crate::websocket_server::WebSocketServer;
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;
use tracing::{info, trace, warn};

/// 默认管理服务端口。
pub const DEFAULT_ADMIN_PORT: u16 = 8888;

const ADMIN_BIND_ADDRESS: &str = "127.0.0.1";
const MAX_HTTP_REQUEST_SIZE: usize = 64 * 1024;

/// 管理页面 HTTP 服务器。
pub struct AdminServer {
    port: u16,
    state: Arc<Mutex<AdminState>>,
}

struct AdminState {
    config_path: PathBuf,
    preset_store_path: PathBuf,
    config: Config,
    websocket: WebSocketServer,
    keyboard_hook: Option<KeyboardHook>,
    mouse_hook: Option<MouseHook>,
    presets: Vec<Preset>,
    server_started_at: Instant,
    service_started_at: Option<Instant>,
    errors_count: u64,
    warnings_count: u64,
}

struct HttpRequest {
    method: String,
    path: String,
    body: Vec<u8>,
}

#[derive(Debug, Deserialize)]
struct PresetImportRequest {
    file_name: String,
    content: String,
    mode: Option<String>,
}

impl AdminServer {
    /// 创建管理页面 HTTP 服务器。
    ///
    /// 参数:
    /// - `port`: 管理 API 监听端口
    /// - `config_path`: TOML 配置文件路径
    /// - `config`: 当前运行配置
    /// - `websocket`: 推流 WebSocket 服务句柄
    /// - `keyboard_hook`: 已启动的键盘 Hook
    /// - `mouse_hook`: 已启动的鼠标 Hook
    ///
    /// 返回:
    /// - 可启动的 [`AdminServer`]
    pub fn new(
        port: u16,
        config_path: PathBuf,
        config: Config,
        websocket: WebSocketServer,
        keyboard_hook: KeyboardHook,
        mouse_hook: MouseHook,
    ) -> Self {
        let now = Instant::now();
        let preset_store_path = preset_store_path(&config_path);
        let presets = match load_presets_from_store(&preset_store_path) {
            Ok(presets) => presets,
            Err(error) => {
                warn!(%error, path = %preset_store_path.display(), "failed to load preset store");
                Vec::new()
            }
        };
        Self {
            port,
            state: Arc::new(Mutex::new(AdminState {
                preset_store_path,
                config_path,
                config,
                websocket,
                keyboard_hook: Some(keyboard_hook),
                mouse_hook: Some(mouse_hook),
                presets,
                server_started_at: now,
                service_started_at: Some(now),
                errors_count: 0,
                warnings_count: 0,
            })),
        }
    }

    /// 启动管理页面服务。
    ///
    /// 返回:
    /// - TCP 监听成功建立时返回 `Ok(())`
    pub async fn start(&self) -> Result<()> {
        let endpoint = format!("{ADMIN_BIND_ADDRESS}:{}", self.port);
        let listener = TcpListener::bind(&endpoint).await?;
        let state = Arc::clone(&self.state);
        let admin_port = self.port;

        tokio::spawn(async move {
            info!(%endpoint, "admin http server listening");
            loop {
                match listener.accept().await {
                    Ok((stream, peer_addr)) => {
                        let state = Arc::clone(&state);
                        tokio::spawn(async move {
                            if let Err(error) =
                                handle_admin_connection(stream, state, admin_port).await
                            {
                                warn!(%peer_addr, %error, "admin http client handler stopped");
                            }
                        });
                    }
                    Err(error) => {
                        warn!(%error, "failed to accept admin http connection");
                    }
                }
            }
        });

        Ok(())
    }

    /// 停止管理服务持有的输入监听资源。
    ///
    /// 返回:
    /// - Hook 资源清理完成时返回 `Ok(())`
    pub async fn stop(&self) -> Result<()> {
        let mut state = self.state.lock().await;
        stop_input_service(&mut state).await
    }
}

impl AdminState {
    /// 判断输入监听服务是否正在运行。
    ///
    /// 返回:
    /// - 任一输入 Hook 仍存在时返回 `true`
    fn service_running(&self) -> bool {
        self.keyboard_hook.is_some() || self.mouse_hook.is_some()
    }
}

/// 处理单个管理 HTTP 连接。
///
/// 参数:
/// - `stream`: TCP 连接
/// - `state`: 管理服务共享状态
/// - `admin_port`: 管理服务端口
///
/// 返回:
/// - 响应成功写入时返回 `Ok(())`
async fn handle_admin_connection(
    mut stream: TcpStream,
    state: Arc<Mutex<AdminState>>,
    admin_port: u16,
) -> Result<()> {
    let request = read_http_request(&mut stream).await?;
    let request_path = request.path.clone();
    let response = match route_admin_request(request, Arc::clone(&state), admin_port).await {
        Ok(response) => response,
        Err(error) => {
            increment_error_count(&state).await;
            api_error_response(
                "500 Internal Server Error",
                "INTERNAL_ERROR",
                &error.to_string(),
            )?
        }
    };

    stream.write_all(&response).await?;
    stream.shutdown().await?;
    trace!(request_path, "admin http response sent");
    Ok(())
}

/// 读取并解析 HTTP 请求。
///
/// 参数:
/// - `stream`: TCP 连接
///
/// 返回:
/// - 解析后的 [`HttpRequest`]
async fn read_http_request(stream: &mut TcpStream) -> Result<HttpRequest> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];

    loop {
        let size = stream.read(&mut chunk).await?;
        if size == 0 {
            break;
        }

        buffer.extend_from_slice(&chunk[..size]);
        if buffer.len() > MAX_HTTP_REQUEST_SIZE {
            return Err(Error::UI("admin http request is too large".to_string()));
        }

        if let Some(header_end) = find_header_end(&buffer) {
            let headers = String::from_utf8_lossy(&buffer[..header_end]);
            let content_length = parse_content_length(&headers);
            if buffer.len() >= header_end + 4 + content_length {
                break;
            }
        }
    }

    parse_http_request(&buffer)
}

/// 路由管理 API 请求。
///
/// 参数:
/// - `request`: HTTP 请求
/// - `state`: 管理服务共享状态
/// - `admin_port`: 管理服务端口
///
/// 返回:
/// - 完整 HTTP 响应字节
async fn route_admin_request(
    request: HttpRequest,
    state: Arc<Mutex<AdminState>>,
    admin_port: u16,
) -> Result<Vec<u8>> {
    let route_path = route_path(&request.path);
    match (request.method.as_str(), route_path) {
        ("OPTIONS", _) => Ok(cors_preflight_response()),
        ("GET", "/") | ("GET", "/admin") | ("GET", "/admin/") => admin_page_response().await,
        ("GET", path) if path.starts_with("/admin/") => admin_static_response(path).await,
        ("GET", "/version") | ("GET", "/api/version") => version_response(),
        ("GET", "/api/config") => config_response(&state).await,
        ("POST", "/api/config") => save_config_response(&state, &request.body).await,
        ("POST", "/api/config/reload") => reload_config_response(&state).await,
        ("GET", "/api/status") => status_response(&state).await,
        ("GET", "/api/network/ip") => network_response(&state, admin_port).await,
        ("GET", "/api/preview") => preview_response(&state).await,
        ("GET", "/api/logs") => logs_response(),
        ("DELETE", "/api/logs") => logs_cleared_response(),
        ("GET", "/api/preset/list") => preset_list_response(&state).await,
        ("POST", "/api/preset/import") => preset_import_response(&state, &request.body).await,
        ("POST", "/api/service/start") => start_service_response(&state).await,
        ("POST", "/api/service/stop") => stop_service_response(&state).await,
        _ => api_error_response("404 Not Found", "NOT_FOUND", "resource not found"),
    }
}

/// 构造管理页 HTML 响应。
///
/// 返回:
/// - 管理页 HTML 响应
async fn admin_page_response() -> Result<Vec<u8>> {
    let admin_root = locate_admin_root();
    let index_path = admin_root.join("index.html");
    let body = tokio::fs::read(&index_path).await.unwrap_or_else(|_| {
        format!(
            "<!doctype html><html lang=\"zh-CN\"><head><meta charset=\"utf-8\"><title>WebKeyLayer</title></head><body><main><h1>WebKeyLayer</h1><p>管理页面资源未找到。</p></main></body></html>"
        )
        .into_bytes()
    });
    Ok(http_response("200 OK", "text/html; charset=utf-8", body))
}

/// 构造管理页静态资源响应。
///
/// 参数:
/// - `request_path`: 请求路径
///
/// 返回:
/// - 静态资源 HTTP 响应
async fn admin_static_response(request_path: &str) -> Result<Vec<u8>> {
    let admin_root = locate_admin_root();
    let Some(file_path) = admin_file_path(request_path, &admin_root) else {
        return api_error_response("404 Not Found", "NOT_FOUND", "resource not found");
    };

    match tokio::fs::read(&file_path).await {
        Ok(body) => Ok(http_response(
            "200 OK",
            static_content_type(&file_path),
            body,
        )),
        Err(_) => api_error_response("404 Not Found", "NOT_FOUND", "resource not found"),
    }
}

/// 构造版本响应。
///
/// 返回:
/// - 版本信息 JSON 响应
fn version_response() -> Result<Vec<u8>> {
    api_success_response(
        "success",
        Some(json!({
            "name": crate::APP_NAME,
            "version": crate::VERSION,
            "protocol_version": "1.0",
        })),
    )
}

/// 构造配置读取响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
///
/// 返回:
/// - 当前配置 JSON 响应
async fn config_response(state: &Arc<Mutex<AdminState>>) -> Result<Vec<u8>> {
    let state = state.lock().await;
    api_success_response("success", Some(json!(state.config)))
}

/// 构造配置保存响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
/// - `body`: 请求体 JSON 字节
///
/// 返回:
/// - 配置保存结果 JSON 响应
async fn save_config_response(state: &Arc<Mutex<AdminState>>, body: &[u8]) -> Result<Vec<u8>> {
    let patch = parse_json_body(body)?;
    let mut state = state.lock().await;
    let previous_network = state.config.network.clone();
    let mut merged_config = serde_json::to_value(&state.config)?;
    merge_json_object(&mut merged_config, patch);
    let updated_config: Config = serde_json::from_value(merged_config)?;
    let requires_restart = previous_network.port != updated_config.network.port
        || previous_network.bind_address != updated_config.network.bind_address;

    let config_path = state.config_path.to_string_lossy().into_owned();
    ConfigLoader::save(&config_path, &updated_config)?;
    state.config = updated_config.clone();

    api_success_response(
        "Configuration saved successfully",
        Some(json!({
            "config": updated_config,
            "requires_restart": requires_restart,
        })),
    )
}

/// 构造配置重载响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
///
/// 返回:
/// - 配置重载结果 JSON 响应
async fn reload_config_response(state: &Arc<Mutex<AdminState>>) -> Result<Vec<u8>> {
    let config_path = {
        let state = state.lock().await;
        state.config_path.clone()
    };

    let config_path_text = config_path.to_string_lossy().into_owned();
    let config = ConfigLoader::load(&config_path_text)?;
    let mut state = state.lock().await;
    state.config = config.clone();

    api_success_response("Configuration reloaded", Some(json!(config)))
}

/// 构造运行状态响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
///
/// 返回:
/// - 服务状态 JSON 响应
async fn status_response(state: &Arc<Mutex<AdminState>>) -> Result<Vec<u8>> {
    let state = state.lock().await;
    api_success_response(
        "success",
        Some(json!({
            "service_running": state.service_running(),
            "keyboard_hook_active": state.keyboard_hook.is_some(),
            "mouse_hook_active": state.mouse_hook.is_some(),
            "connected_clients": state.websocket.connected_clients(),
            "websocket_server_uptime_ms": elapsed_millis(state.server_started_at),
            "service_uptime_ms": state.service_started_at.map(elapsed_millis).unwrap_or(0),
            "latency_stats": {
                "min_ms": 0,
                "max_ms": 0,
                "avg_ms": 0,
            },
            "errors_count": state.errors_count,
            "warnings_count": state.warnings_count,
            "config_path": state.config_path,
        })),
    )
}

/// 构造网络信息响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
/// - `admin_port`: 管理服务端口
///
/// 返回:
/// - 本机连接地址 JSON 响应
async fn network_response(state: &Arc<Mutex<AdminState>>, admin_port: u16) -> Result<Vec<u8>> {
    let state = state.lock().await;
    let local_ip = get_local_ip();
    let public_host = local_ip.clone().unwrap_or_else(|| "127.0.0.1".to_string());
    let local_ips = local_ip.into_iter().collect::<Vec<_>>();
    let websocket_port = state.config.network.port;

    api_success_response(
        "success",
        Some(json!({
            "local_ips": local_ips,
            "websocket_port": websocket_port,
            "admin_port": admin_port,
            "connection_url": format!("ws://{}:{websocket_port}/stream", public_host),
            "stream_url": format!("http://{}:{websocket_port}/public/", public_host),
            "admin_url": format!("http://127.0.0.1:{admin_port}"),
        })),
    )
}

/// 构造实时预览响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
///
/// 返回:
/// - 当前预览状态 JSON 响应
async fn preview_response(state: &Arc<Mutex<AdminState>>) -> Result<Vec<u8>> {
    let state = state.lock().await;
    let pressed_keys = state
        .keyboard_hook
        .as_ref()
        .map(KeyboardHook::pressed_keys)
        .unwrap_or_default();

    api_success_response(
        "success",
        Some(json!({
            "current_layout": state.config.preset.layout,
            "pressed_keys": pressed_keys,
            "all_elements": active_preset_elements(&state),
        })),
    )
}

/// 构造日志查询响应。
///
/// 返回:
/// - 当前日志查询结果 JSON 响应
fn logs_response() -> Result<Vec<u8>> {
    api_success_response("Log collection is not enabled yet", Some(json!([])))
}

/// 构造日志清理响应。
///
/// 返回:
/// - 日志清理结果 JSON 响应
fn logs_cleared_response() -> Result<Vec<u8>> {
    api_success_response("Logs cleared", Some(json!([])))
}

/// 构造预设列表响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
///
/// 返回:
/// - 当前可用预设列表 JSON 响应
async fn preset_list_response(state: &Arc<Mutex<AdminState>>) -> Result<Vec<u8>> {
    let state = state.lock().await;
    let presets = state.presets.iter().map(preset_summary).collect::<Vec<_>>();
    api_success_response("success", Some(json!(presets)))
}

/// 构造预设导入响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
/// - `body`: JSON 请求体
///
/// 返回:
/// - 预设导入结果 JSON 响应
async fn preset_import_response(state: &Arc<Mutex<AdminState>>, body: &[u8]) -> Result<Vec<u8>> {
    let request: PresetImportRequest = serde_json::from_slice(body)?;
    let mode = import_mode_from_text(request.mode.as_deref())?;
    let (preset, warnings) =
        PresetImporter::import_content(&request.file_name, &request.content, mode)?;
    let summary = preset_summary(&preset);
    let preset_name = preset.name.clone();

    let mut state = state.lock().await;
    state
        .presets
        .retain(|existing| existing.name != preset_name);
    state.config.preset.layout = preset_name;
    let config_path = state.config_path.to_string_lossy().into_owned();
    ConfigLoader::save(&config_path, &state.config)?;
    state.warnings_count = state
        .warnings_count
        .saturating_add(warnings.len().try_into().unwrap_or(u64::MAX));
    state.presets.push(preset);
    save_presets_to_store(&state.preset_store_path, &state.presets)?;

    api_success_response(
        "Preset imported successfully",
        Some(json!({
            "preset": summary,
            "warnings": warnings,
        })),
    )
}

/// 构造服务启动响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
///
/// 返回:
/// - 服务启动结果 JSON 响应
async fn start_service_response(state: &Arc<Mutex<AdminState>>) -> Result<Vec<u8>> {
    let mut state = state.lock().await;
    if state.service_running() {
        return api_success_response("Service already running", Some(service_state_json(&state)));
    }

    let websocket = state.websocket.clone();
    let mut keyboard_hook = KeyboardHook::new()?;
    keyboard_hook.start(websocket.clone()).await?;

    let mut mouse_hook = MouseHook::new()?;
    if let Err(error) = mouse_hook.start(websocket).await {
        let _ = keyboard_hook.stop().await;
        return Err(error);
    }

    state.keyboard_hook = Some(keyboard_hook);
    state.mouse_hook = Some(mouse_hook);
    state.service_started_at = Some(Instant::now());

    api_success_response(
        "Service started successfully",
        Some(service_state_json(&state)),
    )
}

/// 构造服务停止响应。
///
/// 参数:
/// - `state`: 管理服务共享状态
///
/// 返回:
/// - 服务停止结果 JSON 响应
async fn stop_service_response(state: &Arc<Mutex<AdminState>>) -> Result<Vec<u8>> {
    let mut state = state.lock().await;
    if !state.service_running() {
        return api_success_response("Service already stopped", Some(service_state_json(&state)));
    }

    stop_input_service(&mut state).await?;
    api_success_response(
        "Service stopped successfully",
        Some(service_state_json(&state)),
    )
}

/// 停止输入监听服务。
///
/// 参数:
/// - `state`: 管理状态
///
/// 返回:
/// - Hook 停止完成时返回 `Ok(())`
async fn stop_input_service(state: &mut AdminState) -> Result<()> {
    if let Some(mut mouse_hook) = state.mouse_hook.take() {
        mouse_hook.stop().await?;
    }

    if let Some(mut keyboard_hook) = state.keyboard_hook.take() {
        keyboard_hook.stop().await?;
    }

    state.service_started_at = None;
    Ok(())
}

/// 构造服务状态片段。
///
/// 参数:
/// - `state`: 管理状态
///
/// 返回:
/// - 可序列化的服务状态 JSON
fn service_state_json(state: &AdminState) -> Value {
    json!({
        "service_running": state.service_running(),
        "keyboard_hook_active": state.keyboard_hook.is_some(),
        "mouse_hook_active": state.mouse_hook.is_some(),
    })
}

/// 根据当前配置返回当前预设元素。
///
/// 参数:
/// - `state`: 管理状态
///
/// 返回:
/// - 当前激活预设元素 JSON
fn active_preset_elements(state: &AdminState) -> Value {
    state
        .presets
        .iter()
        .find(|preset| preset.name == state.config.preset.layout)
        .map(|preset| json!(preset.elements))
        .unwrap_or_else(|| json!([]))
}

/// 构造预设列表摘要。
///
/// 参数:
/// - `preset`: 内部预设模型
///
/// 返回:
/// - 管理 API 使用的预设摘要 JSON
fn preset_summary(preset: &Preset) -> Value {
    json!({
        "name": preset.name,
        "version": preset.version,
        "width": preset.width,
        "height": preset.height,
        "elements_count": preset.elements.len(),
    })
}

/// 从请求文本解析预设导入模式。
///
/// 参数:
/// - `mode`: 可选导入模式文本
///
/// 返回:
/// - 导入模式
fn import_mode_from_text(mode: Option<&str>) -> Result<ImportMode> {
    match mode.unwrap_or("strict") {
        "strict" => Ok(ImportMode::Strict),
        "lenient" => Ok(ImportMode::Lenient),
        other => Err(Error::UI(format!("unsupported import mode: {other}"))),
    }
}

/// 返回预设持久化文件路径。
///
/// 参数:
/// - `config_path`: 配置文件路径
///
/// 返回:
/// - 与配置文件同目录的 `presets.json`
fn preset_store_path(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("presets.json")
}

/// 从持久化文件加载预设列表。
///
/// 参数:
/// - `store_path`: 预设持久化文件路径
///
/// 返回:
/// - 已保存的预设列表；文件不存在时返回空列表
fn load_presets_from_store(store_path: &Path) -> Result<Vec<Preset>> {
    if !store_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(store_path)?;
    let presets = serde_json::from_str::<Vec<Preset>>(&content)?;
    Ok(presets)
}

/// 保存预设列表到持久化文件。
///
/// 参数:
/// - `store_path`: 预设持久化文件路径
/// - `presets`: 需要保存的预设列表
///
/// 返回:
/// - 写入成功时返回 `Ok(())`
fn save_presets_to_store(store_path: &Path, presets: &[Preset]) -> Result<()> {
    if let Some(parent) = store_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(presets)?;
    std::fs::write(store_path, content)?;
    Ok(())
}

/// 解析 HTTP 请求字节。
///
/// 参数:
/// - `buffer`: 原始 HTTP 请求字节
///
/// 返回:
/// - 解析后的 [`HttpRequest`]
fn parse_http_request(buffer: &[u8]) -> Result<HttpRequest> {
    let header_end = find_header_end(buffer)
        .ok_or_else(|| Error::UI("invalid admin http request".to_string()))?;
    let headers = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = headers.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| Error::UI("missing admin http request line".to_string()))?;
    let mut parts = request_line.split_whitespace();
    let method = parts
        .next()
        .ok_or_else(|| Error::UI("missing admin http method".to_string()))?
        .to_string();
    let path = parts
        .next()
        .ok_or_else(|| Error::UI("missing admin http path".to_string()))?
        .to_string();
    let content_length = parse_content_length(&headers);
    let body_start = header_end + 4;
    let body_end = body_start + content_length;
    if buffer.len() < body_end {
        return Err(Error::UI("incomplete admin http body".to_string()));
    }

    Ok(HttpRequest {
        method,
        path,
        body: buffer[body_start..body_end].to_vec(),
    })
}

/// 查找 HTTP 头部结束位置。
///
/// 参数:
/// - `buffer`: HTTP 请求字节
///
/// 返回:
/// - `\r\n\r\n` 前的字节偏移
fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

/// 解析 Content-Length 头。
///
/// 参数:
/// - `headers`: HTTP 头部文本
///
/// 返回:
/// - 请求体字节数；缺失或非法时返回 `0`
fn parse_content_length(headers: &str) -> usize {
    headers
        .lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            if name.eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

/// 去除查询参数后的路由路径。
///
/// 参数:
/// - `path`: 请求路径
///
/// 返回:
/// - 不包含查询参数的路径
fn route_path(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

/// 解析 JSON 请求体。
///
/// 参数:
/// - `body`: 请求体字节
///
/// 返回:
/// - JSON 值；空请求体返回空对象
fn parse_json_body(body: &[u8]) -> Result<Value> {
    if body.is_empty() {
        Ok(json!({}))
    } else {
        Ok(serde_json::from_slice(body)?)
    }
}

/// 递归合并 JSON 对象。
///
/// 参数:
/// - `target`: 被修改的目标对象
/// - `patch`: 新对象片段
fn merge_json_object(target: &mut Value, patch: Value) {
    match (target, patch) {
        (Value::Object(target), Value::Object(patch)) => {
            for (key, value) in patch {
                match target.get_mut(&key) {
                    Some(target_value) => merge_json_object(target_value, value),
                    None => {
                        target.insert(key, value);
                    }
                }
            }
        }
        (target, patch) => {
            *target = patch;
        }
    }
}

/// 构造成功 API 响应。
///
/// 参数:
/// - `message`: 响应消息
/// - `data`: 可选响应数据
///
/// 返回:
/// - 完整 HTTP 响应字节
fn api_success_response(message: &str, data: Option<Value>) -> Result<Vec<u8>> {
    let mut body = json!({
        "code": 0,
        "message": message,
    });
    if let Some(data) = data {
        body["data"] = data;
    }
    json_response("200 OK", body)
}

/// 构造错误 API 响应。
///
/// 参数:
/// - `status`: HTTP 状态文本
/// - `error_code`: API 错误码
/// - `message`: 错误说明
///
/// 返回:
/// - 完整 HTTP 响应字节
fn api_error_response(status: &str, error_code: &str, message: &str) -> Result<Vec<u8>> {
    json_response(
        status,
        json!({
            "code": -1,
            "message": message,
            "error_code": error_code,
        }),
    )
}

/// 构造 JSON HTTP 响应。
///
/// 参数:
/// - `status`: HTTP 状态文本
/// - `body`: JSON 响应体
///
/// 返回:
/// - 完整 HTTP 响应字节
fn json_response(status: &str, body: Value) -> Result<Vec<u8>> {
    let body = serde_json::to_vec(&body)?;
    Ok(http_response(
        status,
        "application/json; charset=utf-8",
        body,
    ))
}

/// 构造 CORS 预检响应。
///
/// 返回:
/// - 完整 HTTP 响应字节
fn cors_preflight_response() -> Vec<u8> {
    http_response("204 No Content", "text/plain; charset=utf-8", Vec::new())
}

/// 构造普通 HTTP 响应。
///
/// 参数:
/// - `status`: HTTP 状态文本
/// - `content_type`: 响应 MIME 类型
/// - `body`: 响应体字节
///
/// 返回:
/// - 完整 HTTP 响应字节
fn http_response(status: &str, content_type: &str, body: Vec<u8>) -> Vec<u8> {
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\n\r\n",
        body.len()
    );
    [headers.as_bytes(), &body].concat()
}

/// 定位本地管理页静态资源目录。
///
/// 返回:
/// - `frontend/admin` 目录路径
fn locate_admin_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let current_dir = std::env::current_dir().unwrap_or_else(|_| manifest_dir.clone());
    let candidates = [
        current_dir.join("frontend").join("admin"),
        current_dir.join("..").join("frontend").join("admin"),
        manifest_dir.join("..").join("frontend").join("admin"),
    ];

    candidates
        .iter()
        .find(|candidate| candidate.exists())
        .cloned()
        .unwrap_or_else(|| manifest_dir.join("..").join("frontend").join("admin"))
}

/// 将 `/admin/...` 请求路径映射到本地静态文件。
///
/// 参数:
/// - `request_path`: 请求路径
/// - `admin_root`: 管理页静态资源根目录
///
/// 返回:
/// - 安全的本地文件路径；非法路径返回 `None`
fn admin_file_path(request_path: &str, admin_root: &Path) -> Option<PathBuf> {
    let relative = request_path.strip_prefix("/admin/")?;
    if relative.is_empty() {
        return Some(admin_root.join("index.html"));
    }

    let mut safe_path = PathBuf::new();
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => safe_path.push(part),
            _ => return None,
        }
    }

    Some(admin_root.join(safe_path))
}

/// 返回静态资源 Content-Type。
///
/// 参数:
/// - `file_path`: 静态文件路径
///
/// 返回:
/// - MIME 类型
fn static_content_type(file_path: &Path) -> &'static str {
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

/// 计算从指定时间点到现在经过的毫秒数。
///
/// 参数:
/// - `instant`: 起始时间点
///
/// 返回:
/// - 经过毫秒数，溢出时截断为 [`u64::MAX`]
fn elapsed_millis(instant: Instant) -> u64 {
    instant.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

/// 增加管理 API 错误计数。
///
/// 参数:
/// - `state`: 管理服务共享状态
async fn increment_error_count(state: &Arc<Mutex<AdminState>>) {
    let mut state = state.lock().await;
    state.errors_count = state.errors_count.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use super::{
        load_presets_from_store, preset_store_path, route_admin_request, save_presets_to_store,
        AdminServer, AdminState, HttpRequest,
    };
    use crate::config::Config;
    use crate::keyboard_hook::KeyboardHook;
    use crate::mouse_hook::MouseHook;
    use crate::preset::schema::{Position, Preset, PresetElement, TextureMapping};
    use crate::websocket_server::{WebSocketConfig, WebSocketServer};
    use serde_json::{json, Value};
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Instant;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn admin_route_serves_management_page_html() {
        let state = test_state();
        let response = route_admin_request(
            HttpRequest {
                method: "GET".to_string(),
                path: "/admin".to_string(),
                body: Vec::new(),
            },
            state,
            8888,
        )
        .await
        .unwrap();

        let text = String::from_utf8(response).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK"));
        assert!(text.contains("Content-Type: text/html; charset=utf-8"));
        assert!(text.contains("WebKeyLayer"));
    }

    #[tokio::test]
    async fn preset_import_accepts_uploaded_json_content_and_updates_list() {
        let state = test_state();
        let preset_json = r#"{
            "overlay_width": 160,
            "overlay_height": 90,
            "elements": [
                {
                    "type": 0,
                    "pos": [0, 0],
                    "id": "base",
                    "z_level": 0,
                    "mapping": [0, 0, 160, 90]
                },
                {
                    "type": 2,
                    "pos": [0, 0],
                    "id": "unsupported",
                    "z_level": 0,
                    "mapping": [0, 0, 16, 16],
                    "code": 0
                },
                {
                    "type": 1,
                    "pos": [20, 20],
                    "id": "a",
                    "z_level": 1,
                    "mapping": [16, 0, 24, 24],
                    "code": 30
                }
            ]
        }"#;

        let body = serde_json::to_vec(&json!({
            "file_name": "wasd-test.json",
            "mode": "lenient",
            "content": preset_json,
        }))
        .unwrap();

        let import_response = route_admin_request(
            HttpRequest {
                method: "POST".to_string(),
                path: "/api/preset/import".to_string(),
                body,
            },
            Arc::clone(&state),
            8888,
        )
        .await
        .unwrap();
        let import_body = response_json(import_response);
        assert_eq!(import_body["code"], 0);
        assert_eq!(import_body["data"]["preset"]["name"], "wasd-test");
        assert_eq!(import_body["data"]["preset"]["elements_count"], 2);
        assert_eq!(import_body["data"]["warnings"].as_array().unwrap().len(), 1);

        let list_response = route_admin_request(
            HttpRequest {
                method: "GET".to_string(),
                path: "/api/preset/list".to_string(),
                body: Vec::new(),
            },
            state,
            8888,
        )
        .await
        .unwrap();
        let list_body = response_json(list_response);
        assert_eq!(list_body["data"].as_array().unwrap().len(), 1);
        assert_eq!(list_body["data"][0]["name"], "wasd-test");
    }

    #[tokio::test]
    async fn preset_import_persists_presets_and_selected_layout() {
        let dir = unique_temp_dir("preset-store");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.toml");
        crate::config::ConfigLoader::save(config_path.to_str().unwrap(), &Config::default())
            .unwrap();
        let state = test_state_with_config_path(config_path.clone());
        let preset_json = r#"{
            "overlay_width": 80,
            "overlay_height": 40,
            "elements": [
                {
                    "type": 1,
                    "pos": [4, 8],
                    "id": "w",
                    "z_level": 1,
                    "mapping": [0, 0, 16, 16],
                    "code": 17
                }
            ]
        }"#;
        let body = serde_json::to_vec(&json!({
            "file_name": "persistent.json",
            "mode": "strict",
            "content": preset_json,
        }))
        .unwrap();

        let import_response = route_admin_request(
            HttpRequest {
                method: "POST".to_string(),
                path: "/api/preset/import".to_string(),
                body,
            },
            state,
            8888,
        )
        .await
        .unwrap();
        assert_eq!(response_json(import_response)["code"], 0);

        let saved_presets = load_presets_from_store(&preset_store_path(&config_path)).unwrap();
        assert_eq!(saved_presets.len(), 1);
        assert_eq!(saved_presets[0].name, "persistent");

        let saved_config =
            crate::config::ConfigLoader::load(config_path.to_str().unwrap()).unwrap();
        assert_eq!(saved_config.preset.layout, "persistent");

        let _ = std::fs::remove_dir_all(dir);
    }

    #[tokio::test]
    async fn admin_static_route_serves_assets() {
        let state = test_state();
        let response = route_admin_request(
            HttpRequest {
                method: "GET".to_string(),
                path: "/admin/js/admin.js".to_string(),
                body: Vec::new(),
            },
            state,
            8888,
        )
        .await
        .unwrap();

        let text = String::from_utf8(response).unwrap();
        assert!(text.starts_with("HTTP/1.1 200 OK"));
        assert!(text.contains("Content-Type: application/javascript; charset=utf-8"));
        assert!(text.contains("apiGet"));
    }

    #[tokio::test]
    async fn admin_server_loads_existing_preset_store_on_startup() {
        let dir = unique_temp_dir("startup-preset-store");
        std::fs::create_dir_all(&dir).unwrap();
        let config_path = dir.join("config.toml");
        crate::config::ConfigLoader::save(config_path.to_str().unwrap(), &Config::default())
            .unwrap();
        let store_path = preset_store_path(&config_path);
        save_presets_to_store(&store_path, &[sample_preset("restored")]).unwrap();

        let server = AdminServer::new(
            0,
            config_path,
            Config::default(),
            WebSocketServer::new(WebSocketConfig {
                bind_address: "127.0.0.1".to_string(),
                port: 0,
            }),
            KeyboardHook::new().unwrap(),
            MouseHook::new().unwrap(),
        );
        let state = server.state.lock().await;

        assert_eq!(state.presets.len(), 1);
        assert_eq!(state.presets[0].name, "restored");

        let _ = std::fs::remove_dir_all(dir);
    }

    fn test_state() -> Arc<Mutex<AdminState>> {
        test_state_with_config_path(unique_temp_dir("admin-state").join("config.toml"))
    }

    fn test_state_with_config_path(config_path: PathBuf) -> Arc<Mutex<AdminState>> {
        Arc::new(Mutex::new(AdminState {
            preset_store_path: preset_store_path(&config_path),
            config_path,
            config: Config::default(),
            websocket: WebSocketServer::new(WebSocketConfig {
                bind_address: "127.0.0.1".to_string(),
                port: 0,
            }),
            keyboard_hook: None,
            mouse_hook: None,
            presets: Vec::new(),
            server_started_at: Instant::now(),
            service_started_at: None,
            errors_count: 0,
            warnings_count: 0,
        }))
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("webkeylayer-{name}-{stamp}"))
    }

    fn sample_preset(name: &str) -> Preset {
        Preset {
            name: name.to_string(),
            version: "input-overlay".to_string(),
            width: 16,
            height: 16,
            elements: vec![PresetElement {
                id: "key".to_string(),
                element_type: "keyboard".to_string(),
                code: Some(17),
                position: Position { x: 0.0, y: 0.0 },
                texture: TextureMapping {
                    x: 0,
                    y: 0,
                    width: 16,
                    height: 16,
                },
                z_index: 0,
            }],
        }
    }

    fn response_json(response: Vec<u8>) -> Value {
        let text = String::from_utf8(response).unwrap();
        let (_, body) = text.split_once("\r\n\r\n").unwrap();
        serde_json::from_str(body).unwrap()
    }
}
