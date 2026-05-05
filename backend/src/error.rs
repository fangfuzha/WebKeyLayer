//! 统一错误处理

use thiserror::Error;

/// WebKeyLayer 的统一错误类型
#[derive(Error, Debug)]
pub enum Error {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Keyboard hook error: {0}")]
    KeyboardHook(String),

    #[error("Mouse hook error: {0}")]
    MouseHook(String),

    #[error("Preset import error: {0}")]
    PresetImport(String),

    #[error("UI error: {0}")]
    UI(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<String> for Error {
    fn from(msg: String) -> Self {
        Error::Internal(msg)
    }
}

impl From<&str> for Error {
    fn from(msg: &str) -> Self {
        Error::Internal(msg.to_string())
    }
}

/// 便利类型别名
pub type Result<T> = std::result::Result<T, Error>;
