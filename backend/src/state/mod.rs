//! 状态管理层
//!
//! 跨线程按键状态同步和聚合。

pub mod config_state;
pub mod keyboard_state;
pub mod sync;

pub use config_state::ConfigState;
pub use keyboard_state::KeyboardState;
pub use sync::StateSync;

use crate::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 全局应用状态
pub struct AppState {
    pub keyboard: Arc<RwLock<KeyboardState>>,
    pub config: Arc<RwLock<ConfigState>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            keyboard: Arc::new(RwLock::new(KeyboardState::new())),
            config: Arc::new(RwLock::new(ConfigState::new())),
        }
    }

    /// 初始化应用状态
    pub async fn init(&self) -> Result<()> {
        // TODO: 从配置文件加载初始状态
        Ok(())
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
