//! 后端管理页面 API 服务

use crate::Result;

/// 管理页面 HTTP 服务器
pub struct AdminServer {
    port: u16,
}

impl AdminServer {
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// 启动管理页面服务
    pub async fn start(&self) -> Result<()> {
        // TODO: 启动 HTTP 服务器，提供以下 API：
        // - GET /api/config - 获取当前配置
        // - POST /api/config - 保存配置
        // - POST /api/config/reload - 热重载
        // - GET /api/preset/list - 预设列表
        // - POST /api/preset/import - 导入预设
        // - GET /api/preview - 实时预览数据
        // - GET /api/logs - 日志查询
        // - GET /api/status - 运行状态
        // - POST /api/service/start - 启动服务
        // - POST /api/service/stop - 停止服务
        // - GET /api/network/ip - 获取本机 IP 和端口
        Ok(())
    }

    /// 停止服务
    pub async fn stop(&self) -> Result<()> {
        Ok(())
    }
}
