//! UI 层：系统托盘和本地管理网页宿主

pub mod server;
pub mod tray;
pub mod webview;

pub use server::{AdminServer, DEFAULT_ADMIN_PORT};
pub use tray::TrayManager;
pub use webview::WebViewHost;

use crate::Result;

/// UI 管理器
pub struct UIManager {
    tray: Option<TrayManager>,
    webview: Option<WebViewHost>,
    server: Option<AdminServer>,
}

impl UIManager {
    pub fn new() -> Self {
        Self {
            tray: None,
            webview: None,
            server: None,
        }
    }

    /// 初始化 UI 组件
    pub async fn init(&mut self) -> Result<()> {
        // TODO: 初始化托盘、WebView 宿主和管理服务器
        self.tray = Some(TrayManager::new()?);
        self.webview = Some(WebViewHost::new()?);
        Ok(())
    }

    /// 启动 UI
    pub async fn start(&mut self) -> Result<()> {
        if let Some(tray) = &self.tray {
            tray.create_menu()?;
        }

        if let Some(webview) = &self.webview {
            webview.start().await?;
        }

        if let Some(server) = &self.server {
            server.start().await?;
        }

        Ok(())
    }

    /// 停止 UI
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(webview) = &self.webview {
            webview.stop().await?;
        }

        if let Some(server) = &self.server {
            server.stop().await?;
        }

        Ok(())
    }
}

impl Default for UIManager {
    fn default() -> Self {
        Self::new()
    }
}
