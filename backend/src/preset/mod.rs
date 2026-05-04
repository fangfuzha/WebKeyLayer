//! 预设兼容层
//!
//! Input Overlay 预设格式的解析、转换和管理。

pub mod importer;
pub mod renderer;
pub mod schema;

pub use importer::PresetImporter;
pub use schema::Preset;

use crate::Result;

/// 预设管理器
pub struct PresetManager {
    presets: Vec<Preset>,
}

impl PresetManager {
    pub fn new() -> Self {
        Self {
            presets: Vec::new(),
        }
    }

    /// 导入 Input Overlay 预设
    pub fn import_preset(&mut self, preset_path: &str) -> Result<()> {
        // TODO: 使用 PresetImporter 加载预设
        Ok(())
    }

    /// 获取所有预设
    pub fn list_presets(&self) -> &[Preset] {
        &self.presets
    }
}

impl Default for PresetManager {
    fn default() -> Self {
        Self::new()
    }
}
