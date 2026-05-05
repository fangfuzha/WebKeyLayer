//! 键盘状态同步

use std::collections::HashMap;

/// 键盘状态管理
pub struct KeyboardState {
    keys: HashMap<u16, bool>,
}

impl KeyboardState {
    /// 创建空的键盘状态容器。
    ///
    /// 返回:
    /// - 未记录任何按键状态的 [`KeyboardState`]
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
        }
    }

    /// 更新按键状态。
    ///
    /// 参数:
    /// - `keycode`: 虚拟按键码
    /// - `pressed`: 是否处于按下状态
    pub fn update(&mut self, keycode: u16, pressed: bool) {
        self.keys.insert(keycode, pressed);
    }

    /// 查询按键是否处于按下状态。
    ///
    /// 参数:
    /// - `keycode`: 虚拟按键码
    ///
    /// 返回:
    /// - `true` 表示按键处于按下状态
    pub fn is_pressed(&self, keycode: u16) -> bool {
        self.keys.get(&keycode).copied().unwrap_or(false)
    }

    /// 返回当前所有处于按下状态的按键码。
    ///
    /// 返回:
    /// - 已按下按键码列表
    pub fn pressed_keys(&self) -> Vec<u16> {
        self.keys
            .iter()
            .filter(|(_, &pressed)| pressed)
            .map(|(&keycode, _)| keycode)
            .collect()
    }

    /// 清空全部按键状态。
    pub fn clear(&mut self) {
        self.keys.clear();
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::KeyboardState;

    #[test]
    fn tracks_pressed_keys_and_clears_state() {
        let mut state = KeyboardState::new();

        state.update(65, true);
        state.update(66, false);
        state.update(67, true);

        assert!(state.is_pressed(65));
        assert!(!state.is_pressed(66));
        assert!(state.is_pressed(67));

        let mut pressed = state.pressed_keys();
        pressed.sort_unstable();
        assert_eq!(pressed, vec![65, 67]);

        state.clear();
        assert!(state.pressed_keys().is_empty());
    }
}
