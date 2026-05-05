//! 配置管理模块
//!
//! 负责 TOML 配置的加载、验证和热重载。

pub mod loader;
pub mod schema;

pub use loader::ConfigLoader;
pub use schema::Config;

use crate::Result;

/// 配置管理器，处理配置的加载、验证和热重载
pub struct ConfigManager {
    config: Config,
}

impl ConfigManager {
    /// 从文件加载配置
    pub fn load(config_path: &str) -> Result<Self> {
        let config = ConfigLoader::load(config_path)?;
        Ok(Self { config })
    }

    /// 获取当前配置
    pub fn get_config(&self) -> &Config {
        &self.config
    }

    /// 更新配置
    pub fn update(&mut self, config: Config) {
        self.config = config;
    }

    /// 启动配置文件监听（热重载）
    pub async fn watch_config(&self, _config_path: &str) -> Result<()> {
        // TODO: 实现使用 notify crate 的文件监听
        Ok(())
    }
}
