// =============================================================================
// FFI 导出层 — 通过 UniFFI 向 Kotlin/Android 暴露核心功能
// =============================================================================

use std::path::Path;

use crate::{amproj, config, split, ui_parser};

// ── 错误类型 ──────────────────────────────────────────────────────────────────

#[derive(Debug, uniffi::Error)]
pub enum AutomotionError {
    General { reason: String },
}

impl std::fmt::Display for AutomotionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AutomotionError::General { reason } => write!(f, "{reason}"),
        }
    }
}

impl From<String> for AutomotionError {
    fn from(s: String) -> Self {
        AutomotionError::General { reason: s }
    }
}

// ── 数据类型 ──────────────────────────────────────────────────────────────────

#[derive(uniffi::Enum)]
pub enum FfiAmprojType {
    Project,
    Element,
}

#[derive(uniffi::Record)]
pub struct FfiAmprojInfo {
    pub proj_type: FfiAmprojType,
    pub title: String,
}

#[derive(uniffi::Record)]
pub struct FfiCoords {
    pub x: i32,
    pub y: i32,
}

// ── amproj 解析与拆分 ────────────────────────────────────────────────────────

/// 分析 amproj 文件，返回类型和标题
#[uniffi::export]
pub fn analyze_amproj(path: String) -> Result<FfiAmprojInfo, AutomotionError> {
    let (proj_type, title) = amproj::analyze(Path::new(&path))?;
    Ok(FfiAmprojInfo {
        proj_type: match proj_type {
            amproj::AmprojType::Project => FfiAmprojType::Project,
            amproj::AmprojType::Element => FfiAmprojType::Element,
        },
        title,
    })
}

/// 拆分 amproj 到指定目录，返回输出文件路径列表
#[uniffi::export]
pub fn split_amproj_to_dir(
    input_path: String,
    output_dir: String,
) -> Result<Vec<String>, AutomotionError> {
    let paths = split::split_amproj(Path::new(&input_path), Path::new(&output_dir))?;
    Ok(paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

// ── 配置常量 ─────────────────────────────────────────────────────────────────

#[uniffi::export]
pub fn get_popup_dismiss_texts() -> Vec<String> {
    config::POPUP_DISMISS_TEXTS
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[uniffi::export]
pub fn get_package_name() -> String {
    config::PACKAGE_NAME.to_string()
}

#[uniffi::export]
pub fn get_phone_video_dir() -> String {
    config::PHONE_VIDEO_DIR.to_string()
}

// ── UI XML 解析（可选——AccessibilityService 通常直接遍历节点树） ─────────────

/// 在 UI XML 中查找包含指定文本的节点坐标
#[uniffi::export]
pub fn find_ui_element_by_text(xml: String, target_text: String) -> Option<FfiCoords> {
    ui_parser::find_element_by_text(&xml, &target_text).map(|(x, y)| FfiCoords { x, y })
}

/// 在 UI XML 中查找包含指定 resource-id 的节点坐标
#[uniffi::export]
pub fn find_ui_element_by_id(xml: String, target_id: String) -> Option<FfiCoords> {
    ui_parser::find_element_by_id(&xml, &target_id).map(|(x, y)| FfiCoords { x, y })
}
