// =============================================================================
// amproj 修补模块 — 资源路径修复与提取
// =============================================================================
//
// 解决 AM 工程"贴图路径丢失"问题：
//   amproj 内嵌素材，但目标设备上不存在对应文件。
//
// 两种修补模式:
//   1. restore（尊重原路径）: 从 amproj 提取嵌入资源到本地目录
//   2. unify（统一化路径）: 提取资源 + 修改 amproj 内 URI 指向指定设备目录
//
// 资源引用属性:
//   - <media uri="amproj:文件名"> — 资源声明
//   - fillImage="amproj:文件名"   — 形状填充图片
//   - src="amproj:文件名"         — 音频引用

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::logger;

/// 修补模式
#[derive(Debug, Clone, Copy)]
pub enum FixMode {
    /// 提取嵌入资源到本地目录，不修改 amproj
    Restore,
    /// 提取资源 + 修改 amproj URI 指向目标设备路径
    Unify,
}

/// 修补结果
pub struct FixResult {
    /// 提取的资源文件列表（本地路径）
    pub extracted_files: Vec<PathBuf>,
    /// 如果是 unify 模式，输出的修改后 amproj 路径
    pub output_amproj: Option<PathBuf>,
}

/// 执行 amproj 修补
///
/// - `input`: amproj 文件路径
/// - `output_dir`: 资源提取目标目录
/// - `mode`: 修补模式
/// - `device_target_dir`: unify 模式下的设备端目标目录
pub fn fix_amproj(
    input: &Path,
    output_dir: &Path,
    mode: FixMode,
    device_target_dir: Option<&str>,
) -> Result<FixResult, String> {
    logger::step(&format!("开始修补: {}", input.display()));

    let file =
        std::fs::File::open(input).map_err(|e| format!("无法打开文件: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("无法解析 ZIP: {e}"))?;

    // 读取 ZIP 中所有文件
    let mut archive_files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut xml_name = String::new();
    let mut xml_content = String::new();

    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("无法读取 ZIP 条目: {e}"))?;
        let name = entry.name().to_string();
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| format!("读取失败: {e}"))?;

        if name.ends_with(".xml") {
            xml_name = name.clone();
            xml_content = String::from_utf8_lossy(&data).to_string();
        }
        archive_files.push((name, data));
    }

    if xml_content.is_empty() {
        return Err("amproj 中未找到 XML 文件".into());
    }

    // 收集所有嵌入资源引用
    let embedded_refs = collect_embedded_refs(&xml_content);
    logger::info(&format!("发现 {} 个嵌入资源引用", embedded_refs.len()));

    for r in &embedded_refs {
        logger::info(&format!("  资源: {r}"));
    }

    // 确定项目名称（从 XML title 属性或文件名）
    let proj_name = extract_project_name(&xml_content, input);
    let assets_dir = output_dir.join(&proj_name);

    // 创建输出目录
    std::fs::create_dir_all(&assets_dir)
        .map_err(|e| format!("创建输出目录失败: {e}"))?;

    // 提取资源文件
    let mut extracted = Vec::new();
    for (name, data) in &archive_files {
        if name.ends_with(".xml") || name == "manifest.txt" {
            continue;
        }
        if embedded_refs.contains(name.as_str()) {
            let out_path = assets_dir.join(name);
            std::fs::write(&out_path, data)
                .map_err(|e| format!("写入 {name} 失败: {e}"))?;
            extracted.push(out_path);
            logger::info(&format!("  提取: {name}"));
        }
    }

    logger::step(&format!(
        "已提取 {} 个资源到 {}",
        extracted.len(),
        assets_dir.display()
    ));

    // unify 模式：修改 amproj
    let output_amproj = match mode {
        FixMode::Restore => None,
        FixMode::Unify => {
            let target_dir = device_target_dir.unwrap_or("/sdcard/Download/automotion_assets");
            let device_path = format!("{}/{}", target_dir.trim_end_matches('/'), proj_name);

            let new_xml = rewrite_resource_paths(&xml_content, &device_path);
            let out_amproj = output_dir.join(format!("{}_fixed.amproj", proj_name));

            write_fixed_amproj(&out_amproj, &xml_name, &new_xml, &archive_files)?;

            logger::step(&format!(
                "已生成修复后 amproj: {}",
                out_amproj.display()
            ));
            logger::info(&format!(
                "设备端资源路径: file://{}",
                device_path
            ));

            Some(out_amproj)
        }
    };

    Ok(FixResult {
        extracted_files: extracted,
        output_amproj,
    })
}

/// 收集 XML 中所有 `amproj:` 前缀的嵌入资源文件名
fn collect_embedded_refs(xml: &str) -> HashSet<&str> {
    let mut refs = HashSet::new();
    let re = Regex::new(r#"amproj:([^"]+)"#).unwrap();
    for cap in re.captures_iter(xml) {
        refs.insert(cap.get(1).unwrap().as_str());
    }
    refs
}

/// 从 XML title 属性或文件名提取项目名
fn extract_project_name(xml: &str, input: &Path) -> String {
    if let Some(caps) = Regex::new(r#"<scene\s+title="([^"]*)""#)
        .unwrap()
        .captures(xml)
    {
        let title = &caps[1];
        if !title.is_empty() {
            return sanitize_dirname(title);
        }
    }
    input
        .file_stem()
        .map(|s| sanitize_dirname(&s.to_string_lossy()))
        .unwrap_or_else(|| "unknown".to_string())
}

/// 将 XML 中所有 `amproj:filename` 替换为 `file:///device_path/filename`
fn rewrite_resource_paths(xml: &str, device_path: &str) -> String {
    let re = Regex::new(r#"amproj:([^"]+)"#).unwrap();
    re.replace_all(xml, |caps: &regex::Captures| {
        let filename = &caps[1];
        format!("file:///{}/{}", device_path.trim_start_matches('/'), filename)
    })
    .to_string()
}

/// 写入修复后的 amproj ZIP（保留原嵌入资源）
fn write_fixed_amproj(
    out_path: &Path,
    xml_name: &str,
    new_xml: &str,
    archive_files: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let out_file =
        std::fs::File::create(out_path).map_err(|e| format!("创建输出文件失败: {e}"))?;
    let mut zip_writer = zip::ZipWriter::new(out_file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (name, data) in archive_files {
        zip_writer
            .start_file(name, options)
            .map_err(|e| format!("ZIP 写入失败: {e}"))?;

        if name == xml_name {
            // 写入修改后的 XML
            std::io::Write::write_all(&mut zip_writer, new_xml.as_bytes())
                .map_err(|e| format!("写入 XML 失败: {e}"))?;
        } else {
            // 原样保留其他文件（资源+manifest）
            std::io::Write::write_all(&mut zip_writer, data)
                .map_err(|e| format!("写入资源失败: {e}"))?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("ZIP 完成失败: {e}"))?;
    Ok(())
}

fn sanitize_dirname(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    cleaned.trim().replace(' ', "_")
}
