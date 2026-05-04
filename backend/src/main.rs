//! WebKeyLayer 后端主程序入口
//!
//! 负责初始化所有组件并启动主事件循环。

use anyhow::Result;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("webkeylayer_backend=debug".parse()?),
        )
        .init();

    info!("WebKeyLayer backend v{} starting", webkeylayer_backend::VERSION);

    // TODO: 初始化各个核心模块
    // 1. 加载配置文件
    // 2. 初始化键盘 Hook
    // 3. 启动 WebSocket 服务器
    // 4. 启动本地管理页面 WebView 宿主
    // 5. 初始化系统托盘
    // 6. 启动主事件循环

    info!("Application initialized successfully");

    // 保持应用运行
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received");

    Ok(())
}
