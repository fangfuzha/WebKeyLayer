//! 预设数据结构

use serde::{Deserialize, Serialize};

/// 内部统一预设格式
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub version: String,
    pub width: u32,
    pub height: u32,
    pub elements: Vec<PresetElement>,
}

/// 预设中的单个元素
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetElement {
    pub id: String,
    pub element_type: String,
    pub position: Position,
    pub texture: TextureMapping,
    pub z_index: i32,
}

/// 元素位置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

/// 贴图映射（用于从大图中切片）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureMapping {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
