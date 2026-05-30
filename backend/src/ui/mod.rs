//! UI 层：系统托盘和本地管理服务

pub mod server;
pub mod tray;

pub use server::{AdminServer, DEFAULT_ADMIN_PORT};
pub use tray::TrayManager;

use crate::Result;

/// UI 管理器
pub struct UIManager {
    tray: Option<TrayManager>,
    server: Option<AdminServer>,
}

impl UIManager {
    pub fn new() -> Self {
        Self {
            tray: None,
            server: None,
        }
    }

    /// 初始化 UI 组件
    pub async fn init(&mut self) -> Result<()> {
        // TODO: 初始化托盘和管理服务器
        self.tray = Some(TrayManager::new(DEFAULT_ADMIN_PORT)?);
        Ok(())
    }

    /// 启动 UI
    pub async fn start(&mut self) -> Result<()> {
        if let Some(tray) = &mut self.tray {
            tray.create_menu()?;
        }

        if let Some(server) = &self.server {
            server.start().await?;
        }

        Ok(())
    }

    /// 停止 UI
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tray) = &mut self.tray {
            tray.stop()?;
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
