//! 配置数据结构定义

use serde::{Deserialize, Serialize};

/// 主题配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    /// 主题模式：light, dark, high_contrast, system
    pub mode: String,
    /// 主要颜色
    pub primary_color: String,
    /// 高亮颜色
    pub highlight_color: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: "light".to_string(),
            primary_color: "#333333".to_string(),
            highlight_color: "#FF5722".to_string(),
        }
    }
}

/// 预设配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetConfig {
    /// 选中的键盘布局
    pub layout: String,
    /// 按键样式：square, circle, flat, glassmorphism
    pub style: String,
}

impl Default for PresetConfig {
    fn default() -> Self {
        Self {
            layout: "wasd-minimal".to_string(),
            style: "square".to_string(),
        }
    }
}

/// UI 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UIConfig {
    /// 透明度 (0.0-1.0)
    pub transparency: f32,
    /// 缩放比例
    pub scale: f32,
}

impl Default for UIConfig {
    fn default() -> Self {
        Self {
            transparency: 0.9,
            scale: 1.0,
        }
    }
}

/// 网络配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// WebSocket 服务端口
    pub port: u16,
    /// 绑定地址
    pub bind_address: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            bind_address: "0.0.0.0".to_string(),
        }
    }
}

/// 国际化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct I18nConfig {
    /// 语言：zh-CN, en-US
    pub language: String,
}

impl Default for I18nConfig {
    fn default() -> Self {
        Self {
            language: "zh-CN".to_string(),
        }
    }
}

/// 完整配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub theme: ThemeConfig,
    pub preset: PresetConfig,
    pub ui: UIConfig,
    pub network: NetworkConfig,
    pub i18n: I18nConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: ThemeConfig::default(),
            preset: PresetConfig::default(),
            ui: UIConfig::default(),
            network: NetworkConfig::default(),
            i18n: I18nConfig::default(),
        }
    }
}
