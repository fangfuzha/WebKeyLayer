//! 日志与诊断

use tracing::{debug, info, warn};

/// 日志管理器
pub struct LogManager;

impl LogManager {
    /// 初始化日志系统
    pub fn init() {
        // 日志已在 main.rs 中初始化
        info!("Log system initialized");
    }

    /// 记录诊断信息
    pub fn log_diagnostic(message: &str) {
        debug!("Diagnostic: {}", message);
    }

    /// 记录警告信息
    pub fn log_warning(message: &str) {
        warn!("Warning: {}", message);
    }
}
