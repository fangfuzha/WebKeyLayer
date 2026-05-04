//! Input Overlay 预设导入器

use crate::Result;
use serde::{Deserialize, Serialize};

/// Input Overlay 预设元素类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementType {
    KeyboardKey = 1,
    MouseButton = 2,
    Texture = 3,
    Wheel = 4,
    MouseMovement = 5,
    GamepadButton = 6,
    AnalogStick = 7,
    Trigger = 8,
    DPadStick = 9,
    GamepadId = 10,
}

impl ElementType {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            1 => Some(ElementType::KeyboardKey),
            2 => Some(ElementType::MouseButton),
            3 => Some(ElementType::Texture),
            4 => Some(ElementType::Wheel),
            5 => Some(ElementType::MouseMovement),
            6 => Some(ElementType::GamepadButton),
            7 => Some(ElementType::AnalogStick),
            8 => Some(ElementType::Trigger),
            9 => Some(ElementType::DPadStick),
            10 => Some(ElementType::GamepadId),
            _ => None,
        }
    }
}

/// 导入模式
#[derive(Debug, Clone, Copy)]
pub enum ImportMode {
    /// 严格模式：检查所有必需字段，发现缺失时拒绝导入
    Strict,
    /// 宽松模式：允许部分字段缺失，跳过后继续
    Lenient,
}

/// Input Overlay JSON 预设导入器
pub struct PresetImporter;

impl PresetImporter {
    /// 导入 Input Overlay JSON 预设
    pub fn import(json_path: &str, mode: ImportMode) -> Result<()> {
        // TODO: 实现完整的 Input Overlay 预设解析逻辑
        // 1. 解析 JSON 文件
        // 2. 验证必需字段
        // 3. 转换元素类型和字段
        // 4. 根据模式处理兼容性问题
        // 5. 返回转换后的预设或错误
        Ok(())
    }

    /// 验证预设兼容性
    pub fn validate(json_path: &str) -> Result<Vec<String>> {
        // TODO: 验证并返回不兼容项列表
        Ok(Vec::new())
    }
}
