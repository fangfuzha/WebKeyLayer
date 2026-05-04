//! 键盘监听模块
//!
//! 实现 Windows 全局键盘监听，不受窗口焦点影响。
//!
//! # 设计约束
//! - 系统级 Hook（RegisterHotKey 或 SetWindowsHookEx）
//! - 线程安全状态共享（Arc<Mutex<>>）
//! - Linux 预留支持空间

use crate::Result;
use std::collections::HashMap;

/// 键盘状态容器：按键码 -> 是否按下
pub struct KeyboardState {
    keys: HashMap<u16, bool>,
}

impl KeyboardState {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    /// 更新按键状态
    pub fn update(&mut self, keycode: u16, pressed: bool) {
        self.keys.insert(keycode, pressed);
    }

    /// 获取按键状态
    pub fn is_pressed(&self, keycode: u16) -> bool {
        self.keys.get(&keycode).copied().unwrap_or(false)
    }

    /// 获取所有按下的按键列表
    pub fn pressed_keys(&self) -> Vec<u16> {
        self.keys
            .iter()
            .filter(|(_, &pressed)| pressed)
            .map(|(&keycode, _)| keycode)
            .collect()
    }

    /// 清空所有按键状态
    pub fn clear(&mut self) {
        self.keys.clear();
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局键盘 Hook 实现
pub struct KeyboardHook {
    // TODO: 平台特定实现
    state: KeyboardState,
}

impl KeyboardHook {
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: KeyboardState::new(),
        })
    }

    /// 启动键盘监听
    pub async fn start(&mut self) -> Result<()> {
        // TODO: 实现 Windows Hook 初始化
        Ok(())
    }

    /// 停止键盘监听
    pub async fn stop(&mut self) -> Result<()> {
        // TODO: 实现 Hook 清理
        Ok(())
    }

    /// 获取当前按键状态
    pub fn get_state(&self) -> &KeyboardState {
        &self.state
    }

    /// 获取可变按键状态（用于状态更新）
    pub fn get_state_mut(&mut self) -> &mut KeyboardState {
        &mut self.state
    }
}

impl Default for KeyboardHook {
    fn default() -> Self {
        Self::new().expect("Failed to create KeyboardHook")
    }
}
