//! 配置加载器

use super::Config;
use crate::Result;
use std::fs;

pub struct ConfigLoader;

impl ConfigLoader {
    /// 从 TOML 文件加载配置
    pub fn load(config_path: &str) -> Result<Config> {
        let content = fs::read_to_string(config_path)
            .map_err(|e| crate::Error::Config(format!("Failed to read config: {}", e)))?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| crate::Error::Config(format!("Failed to parse config: {}", e)))?;

        Ok(config)
    }

    /// 保存配置到 TOML 文件
    pub fn save(config_path: &str, config: &Config) -> Result<()> {
        let content = toml::to_string_pretty(config)
            .map_err(|e| crate::Error::Config(format!("Failed to serialize config: {}", e)))?;

        fs::write(config_path, content)
            .map_err(|e| crate::Error::Config(format!("Failed to write config: {}", e)))?;

        Ok(())
    }

    /// 生成默认配置文件
    pub fn create_default(config_path: &str) -> Result<Config> {
        let config = Config::default();
        Self::save(config_path, &config)?;
        Ok(config)
    }
}
