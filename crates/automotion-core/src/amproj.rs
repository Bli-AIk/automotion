// =============================================================================
// amproj 分析模块 — 解析 amproj 内部 XML 判定类型
// =============================================================================

use std::io::Read;
use std::path::Path;

use regex::Regex;
use zip::ZipArchive;

use crate::logger;

/// amproj 工程类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AmprojType {
    /// 项目（完整工程，可直接导出视频）
    Project,
    /// 元素（预设/组件，不能直接导出视频）
    Element,
}

impl std::fmt::Display for AmprojType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AmprojType::Project => write!(f, "项目"),
            AmprojType::Element => write!(f, "元素"),
        }
    }
}

/// 分析 amproj 文件，返回其类型和项目标题
/// amproj 是 zip 格式，内含 XML 文件，根节点 <scene> 的 type 属性决定类型：
/// - type="element" → 元素
/// - 无 type 属性或其他值 → 项目
pub fn analyze(path: &Path) -> Result<(AmprojType, String), String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("无法打开文件 {}: {e}", path.display()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("无法解析 zip: {e}"))?;

    // 查找 .xml 文件
    let xml_index = (0..archive.len())
        .find(|&i| {
            archive
                .by_index(i)
                .map(|f| f.name().ends_with(".xml"))
                .unwrap_or(false)
        })
        .ok_or("amproj 中未找到 XML 文件")?;

    let mut xml_file = archive
        .by_index(xml_index)
        .map_err(|e| format!("无法读取 XML: {e}"))?;

    // 只需读取前 2KB 就够分析 <scene> 标签
    let mut buf = vec![0u8; 2048];
    let n = xml_file.read(&mut buf).map_err(|e| format!("读取失败: {e}"))?;
    let header = String::from_utf8_lossy(&buf[..n]);

    // 提取 type 属性
    let proj_type = if let Some(caps) = Regex::new(r#"type="(\w+)""#)
        .unwrap()
        .captures(&header)
    {
        // 要注意区分 <scene type="element"> 和 <media type="image/png">
        // <scene> 标签的 type 在行首区域
        if header.contains(r#"type="element""#)
            && header
                .find(r#"type="element""#)
                .unwrap_or(usize::MAX)
                < header.find("<media").unwrap_or(usize::MAX)
        {
            AmprojType::Element
        } else {
            let val = &caps[1];
            if val == "element" {
                AmprojType::Element
            } else {
                AmprojType::Project
            }
        }
    } else {
        AmprojType::Project
    };

    // 提取标题
    let title = Regex::new(r#"<scene\s+title="([^"]*)""#)
        .unwrap()
        .captures(&header)
        .map(|c| c[1].to_string())
        .unwrap_or_else(|| "未知".to_string());

    logger::info(&format!(
        "amproj 分析: 类型={proj_type}, 标题=\"{title}\""
    ));

    Ok((proj_type, title))
}
