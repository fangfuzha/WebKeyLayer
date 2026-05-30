//! WebKeyLayer 后端主程序入口
//!
//! 负责初始化所有组件并启动主事件循环。

use anyhow::Result;
use std::path::PathBuf;
use tracing::info;
use webkeylayer_backend::config::ConfigLoader;
use webkeylayer_backend::keyboard_hook::KeyboardHook;
use webkeylayer_backend::mouse_hook::MouseHook;
use webkeylayer_backend::ui::{AdminServer, TrayManager, DEFAULT_ADMIN_PORT};
use webkeylayer_backend::websocket_server::{WebSocketConfig, WebSocketServer};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("webkeylayer_backend=debug".parse()?),
        )
        .init();

    info!(
        "WebKeyLayer backend v{} starting",
        webkeylayer_backend::VERSION
    );

    let config_path = default_config_path();
    let config_path_text = config_path.to_string_lossy().into_owned();
    let config = ConfigLoader::load_or_create(&config_path_text)?;

    let websocket = WebSocketServer::new(WebSocketConfig {
        bind_address: config.network.bind_address.clone(),
        port: config.network.port,
    });
    websocket.start().await?;

    info!(
        bind_address = %config.network.bind_address,
        port = config.network.port,
        "WebSocket service started"
    );

    let mut keyboard_hook = KeyboardHook::new()?;
    keyboard_hook.start(websocket.clone()).await?;

    let mut mouse_hook = MouseHook::new()?;
    mouse_hook.start(websocket.clone()).await?;

    let admin_server = AdminServer::new(
        DEFAULT_ADMIN_PORT,
        config_path,
        config.clone(),
        websocket.clone(),
        keyboard_hook,
        mouse_hook,
    );
    admin_server.start().await?;

    info!(
        admin_url = format!("http://127.0.0.1:{}", DEFAULT_ADMIN_PORT),
        "Admin service started"
    );

    let mut tray_manager = TrayManager::new(DEFAULT_ADMIN_PORT)?;
    tray_manager.create_menu()?;
    tray_manager.update_status(true)?;

    info!("Application initialized successfully");

    // 保持应用运行
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received");

    admin_server.stop().await?;
    tray_manager.stop()?;

    Ok(())
}

/// 返回默认配置文件路径。
///
/// 返回:
/// - Windows 优先使用 `%APPDATA%/WebKeyLayer/config.toml`，否则退回当前目录
fn default_config_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("WebKeyLayer")
        .join("config.toml")
}
