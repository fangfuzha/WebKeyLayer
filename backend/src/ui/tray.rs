//! 系统托盘管理

use crate::Result;

/// 系统托盘管理器
pub struct TrayManager {
    // TODO: 平台特定的托盘实现
}

impl TrayManager {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    /// 创建托盘菜单项
    pub fn create_menu(&self) -> Result<()> {
        // TODO: 创建右键菜单：
        // - 打开管理面板
        // - 启动/停止服务
        // - 显示/复制连接地址
        // - 查看日志
        // - 退出程序
        Ok(())
    }

    /// 更新托盘图标状态
    pub fn update_status(&self, running: bool) -> Result<()> {
        // TODO: 根据服务状态更新图标
        Ok(())
    }
}
