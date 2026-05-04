//! 本地 WebView 宿主

use crate::Result;

/// WebView 宿主（用于托管本地管理网页）
pub struct WebViewHost {
    // TODO: 平台特定的 WebView 实现
}

impl WebViewHost {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    /// 启动 WebView 并加载本地管理页面
    pub async fn start(&self) -> Result<()> {
        // TODO: 使用 WebView2 (Windows) 或 wry (Linux) 启动
        // 加载 http://127.0.0.1:8888/admin
        Ok(())
    }

    /// 停止 WebView
    pub async fn stop(&self) -> Result<()> {
        Ok(())
    }
}
