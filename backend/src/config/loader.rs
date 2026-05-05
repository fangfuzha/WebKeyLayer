//! 配置加载器

use super::Config;
use crate::Result;
use std::fs;
use std::path::Path;

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

    /// 加载配置文件；若文件不存在则创建默认配置。
    ///
    /// 参数:
    /// - `config_path`: 配置文件路径
    ///
    /// 返回:
    /// - 已加载或新创建的配置
    pub fn load_or_create(config_path: &str) -> Result<Config> {
        if Path::new(config_path).exists() {
            Self::load(config_path)
        } else {
            Self::create_default(config_path)
        }
    }

    /// 保存配置到 TOML 文件
    pub fn save(config_path: &str, config: &Config) -> Result<()> {
        let content = toml::to_string_pretty(config)
            .map_err(|e| crate::Error::Config(format!("Failed to serialize config: {}", e)))?;

        if let Some(parent) = Path::new(config_path).parent() {
            fs::create_dir_all(parent)
                .map_err(|e| crate::Error::Config(format!("Failed to create config dir: {}", e)))?;
        }

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
