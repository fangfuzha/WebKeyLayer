//! WebKeyLayer 后端库
//!
//! 提供全局键盘监听、WebSocket 事件分发、配置管理和本地 UI 宿主的核心功能。
//!
//! # 核心模块
//!
//! - [`keyboard_hook`]: Windows 全局键盘监听
//! - [`websocket_server`]: WebSocket 事件分发服务
//! - [`config`]: TOML 配置管理与热重载
//! - [`preset`]: Input Overlay 预设兼容性层
//! - [`ui`]: 系统托盘与本地管理网页宿主
//! - [`state`]: 跨线程键盘状态同步
//! - [`error`]: 统一错误处理

pub mod config;
pub mod error;
pub mod keyboard_hook;
pub mod log;
pub mod mouse_hook;
pub mod preset;
pub mod state;
pub mod ui;
pub mod util;
pub mod websocket_server;

pub use error::{Error, Result};

/// 应用版本号
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 应用名称
pub const APP_NAME: &str = env!("CARGO_PKG_NAME");
