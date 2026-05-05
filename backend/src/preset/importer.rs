//! Input Overlay 预设导入器

use crate::preset::schema::{Position, Preset, PresetElement, TextureMapping};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::Path;

/// Input Overlay 预设元素类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ElementType {
    Texture = 0,
    KeyboardKey = 1,
    GamepadButton = 2,
    MouseButton = 3,
    Wheel = 4,
    AnalogStick = 5,
    Trigger = 6,
    GamepadId = 7,
    DPadStick = 8,
    MouseMovement = 9,
}

impl ElementType {
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(ElementType::Texture),
            1 => Some(ElementType::KeyboardKey),
            2 => Some(ElementType::GamepadButton),
            3 => Some(ElementType::MouseButton),
            4 => Some(ElementType::Wheel),
            5 => Some(ElementType::AnalogStick),
            6 => Some(ElementType::Trigger),
            7 => Some(ElementType::GamepadId),
            8 => Some(ElementType::DPadStick),
            9 => Some(ElementType::MouseMovement),
            _ => None,
        }
    }

    fn internal_name(self) -> Option<&'static str> {
        match self {
            ElementType::Texture => Some("texture"),
            ElementType::KeyboardKey => Some("keyboard"),
            ElementType::MouseButton => Some("mouse_button"),
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
    pub fn import(json_path: &str, mode: ImportMode) -> Result<Preset> {
        let (preset, warnings) = convert_input_overlay_file(json_path)?;
        if matches!(mode, ImportMode::Strict) && !warnings.is_empty() {
            return Err(Error::PresetImport(warnings.join("; ")));
        }
        Ok(preset)
    }

    /// 从 JSON 文本导入 Input Overlay 预设。
    ///
    /// 参数:
    /// - `file_name`: 上传文件名，用于生成预设名称
    /// - `content`: Input Overlay JSON 文本
    /// - `mode`: 导入兼容模式
    ///
    /// 返回:
    /// - 内部预设模型与兼容性告警
    pub fn import_content(
        file_name: &str,
        content: &str,
        mode: ImportMode,
    ) -> Result<(Preset, Vec<String>)> {
        let (preset, warnings) = convert_input_overlay_content(file_name, content)?;
        if matches!(mode, ImportMode::Strict) && !warnings.is_empty() {
            return Err(Error::PresetImport(warnings.join("; ")));
        }
        Ok((preset, warnings))
    }

    /// 验证预设兼容性
    pub fn validate(json_path: &str) -> Result<Vec<String>> {
        let (_, warnings) = convert_input_overlay_file(json_path)?;
        Ok(warnings)
    }
}

#[derive(Debug, Deserialize)]
struct InputOverlayPreset {
    overlay_width: u32,
    overlay_height: u32,
    elements: Vec<InputOverlayElement>,
}

#[derive(Debug, Deserialize)]
struct InputOverlayElement {
    #[serde(rename = "type")]
    type_id: i32,
    id: Option<String>,
    pos: Option<[f32; 2]>,
    mapping: Option<[u32; 4]>,
    z_level: Option<Value>,
    code: Option<i32>,
}

fn convert_input_overlay_file(json_path: &str) -> Result<(Preset, Vec<String>)> {
    let content = fs::read_to_string(json_path)?;
    convert_input_overlay_content(&preset_name_from_path(json_path), &content)
}

fn convert_input_overlay_content(
    preset_name: &str,
    content: &str,
) -> Result<(Preset, Vec<String>)> {
    let source: InputOverlayPreset = serde_json::from_str(&content)?;
    let mut warnings = Vec::new();
    let mut elements = Vec::new();

    for (index, source_element) in source.elements.into_iter().enumerate() {
        match convert_element(index, source_element) {
            Ok(element) => elements.push(element),
            Err(warning) => warnings.push(warning),
        }
    }

    Ok((
        Preset {
            name: preset_name_from_path(preset_name),
            version: "input-overlay".to_string(),
            width: source.overlay_width,
            height: source.overlay_height,
            elements,
        },
        warnings,
    ))
}

fn convert_element(
    index: usize,
    source: InputOverlayElement,
) -> std::result::Result<PresetElement, String> {
    let id = source
        .id
        .ok_or_else(|| format!("element #{index} missing required field id"))?;
    let element_type = ElementType::from_i32(source.type_id).ok_or_else(|| {
        format!(
            "element '{id}' uses unknown element type {}",
            source.type_id
        )
    })?;
    let internal_type = element_type.internal_name().ok_or_else(|| {
        format!(
            "element '{id}' uses unsupported element type {}",
            source.type_id
        )
    })?;
    let pos = source
        .pos
        .ok_or_else(|| format!("element '{id}' missing required field pos"))?;
    let mapping = source
        .mapping
        .ok_or_else(|| format!("element '{id}' missing required field mapping"))?;
    let z_index = parse_z_index(source.z_level.as_ref())
        .map_err(|error| format!("element '{id}' has invalid z_level: {error}"))?;
    let code = match element_type {
        ElementType::KeyboardKey | ElementType::MouseButton => Some(
            source
                .code
                .ok_or_else(|| format!("element '{id}' missing required field code"))?,
        ),
        _ => source.code,
    };

    Ok(PresetElement {
        id,
        element_type: internal_type.to_string(),
        code,
        position: Position {
            x: pos[0],
            y: pos[1],
        },
        texture: TextureMapping {
            x: mapping[0],
            y: mapping[1],
            width: mapping[2],
            height: mapping[3],
        },
        z_index,
    })
}

fn parse_z_index(value: Option<&Value>) -> std::result::Result<i32, String> {
    let Some(value) = value else {
        return Ok(0);
    };

    if let Some(number) = value.as_i64() {
        return i32::try_from(number).map_err(|error| error.to_string());
    }

    if let Some(text) = value.as_str() {
        return text.parse::<i32>().map_err(|error| error.to_string());
    }

    Err("expected integer or integer string".to_string())
}

fn preset_name_from_path(json_path: &str) -> String {
    Path::new(json_path)
        .file_stem()
        .and_then(|file_stem| file_stem.to_str())
        .filter(|file_stem| !file_stem.is_empty())
        .unwrap_or("input-overlay")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{ImportMode, PresetImporter};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn imports_supported_input_overlay_elements_into_internal_preset() {
        let path = write_temp_preset(
            "supported",
            r#"{
                "overlay_width": 320,
                "overlay_height": 180,
                "elements": [
                    {
                        "type": 0,
                        "pos": [0, 0],
                        "id": "base",
                        "z_level": 0,
                        "mapping": [0, 0, 320, 180]
                    },
                    {
                        "type": 1,
                        "pos": [20, 30],
                        "id": "w",
                        "z_level": "2",
                        "mapping": [10, 20, 40, 50],
                        "code": 17
                    },
                    {
                        "type": 3,
                        "pos": [90, 30],
                        "id": "lmb",
                        "z_level": 3,
                        "mapping": [60, 20, 30, 50],
                        "code": 1
                    }
                ]
            }"#,
        );

        let preset = PresetImporter::import(path.to_str().unwrap(), ImportMode::Strict).unwrap();

        assert_eq!(preset.name, "supported");
        assert_eq!(preset.version, "input-overlay");
        assert_eq!(preset.width, 320);
        assert_eq!(preset.height, 180);
        assert_eq!(preset.elements.len(), 3);
        assert_eq!(preset.elements[0].element_type, "texture");
        assert_eq!(preset.elements[0].code, None);
        assert_eq!(preset.elements[1].element_type, "keyboard");
        assert_eq!(preset.elements[1].code, Some(17));
        assert_eq!(preset.elements[1].position.x, 20.0);
        assert_eq!(preset.elements[1].texture.width, 40);
        assert_eq!(preset.elements[1].z_index, 2);
        assert_eq!(preset.elements[2].element_type, "mouse_button");
        assert_eq!(preset.elements[2].code, Some(1));

        remove_temp_preset(path);
    }

    #[test]
    fn strict_import_rejects_unsupported_elements() {
        let path = write_temp_preset(
            "unsupported",
            r#"{
                "overlay_width": 100,
                "overlay_height": 100,
                "elements": [
                    {
                        "type": 2,
                        "pos": [10, 10],
                        "id": "gamepad_a",
                        "z_level": 0,
                        "mapping": [0, 0, 20, 20],
                        "code": 0
                    }
                ]
            }"#,
        );

        let error = PresetImporter::import(path.to_str().unwrap(), ImportMode::Strict)
            .expect_err("strict 模式必须拒绝未支持的元素类型");

        assert!(error.to_string().contains("unsupported element type"));

        remove_temp_preset(path);
    }

    #[test]
    fn lenient_import_skips_unsupported_elements_and_reports_warnings() {
        let path = write_temp_preset(
            "lenient",
            r#"{
                "overlay_width": 100,
                "overlay_height": 100,
                "elements": [
                    {
                        "type": 2,
                        "pos": [10, 10],
                        "id": "gamepad_a",
                        "z_level": 0,
                        "mapping": [0, 0, 20, 20],
                        "code": 0
                    },
                    {
                        "type": 1,
                        "pos": [40, 10],
                        "id": "a",
                        "z_level": 0,
                        "mapping": [20, 0, 20, 20],
                        "code": 30
                    }
                ]
            }"#,
        );

        let preset = PresetImporter::import(path.to_str().unwrap(), ImportMode::Lenient).unwrap();
        let warnings = PresetImporter::validate(path.to_str().unwrap()).unwrap();

        assert_eq!(preset.elements.len(), 1);
        assert_eq!(preset.elements[0].id, "a");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("gamepad_a"));

        remove_temp_preset(path);
    }

    fn write_temp_preset(name: &str, content: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("webkeylayer-preset-test-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.json"));
        fs::write(&path, content).unwrap();
        path
    }

    fn remove_temp_preset(path: std::path::PathBuf) {
        if let Some(dir) = path.parent() {
            let _ = fs::remove_dir_all(dir);
        } else {
            let _ = fs::remove_file(path);
        }
    }
}
