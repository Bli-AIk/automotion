// =============================================================================
// amproj 修补模块 — 资源路径修复
// =============================================================================
//
// 解决跨设备传输 amproj 时"贴图丢失"问题。
//
// 原理：Alemon 导入 amproj 时，将 amproj: URI 重映射为 am:SHA1.ext 内部 URI，
// 但此重映射可能因中文文件名等原因失败。本模块直接将 URI 预转换为 am:SHA1.ext
// 格式，绕过导入重映射步骤。
//
// 资源引用属性:
//   - <media uri="amproj:文件名"> — 资源声明
//   - fillImage="amproj:文件名"   — 形状填充图片
//   - src="amproj:文件名"         — 音频引用

use std::io::Read;
use std::path::{Path, PathBuf};

use regex::Regex;

use crate::logger;

/// 修补结果
pub struct FixResult {
    /// 输出的修改后 amproj 路径
    pub output_amproj: PathBuf,
}

/// 执行 amproj 修补：将 amproj: URI 替换为 am:SHA1.ext
///
/// - `input`: amproj 文件路径
/// - `output_dir`: 输出目录
pub fn fix_amproj(input: &Path, output_dir: &Path) -> Result<FixResult, String> {
    logger::step(&format!("开始修补: {}", input.display()));

    let file = std::fs::File::open(input).map_err(|e| format!("无法打开文件: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("无法解析 ZIP: {e}"))?;

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

    // 确定项目名称（从 XML title 属性或文件名）
    let proj_name = extract_project_name(&xml_content, input);

    // 创建输出目录
    std::fs::create_dir_all(output_dir).map_err(|e| format!("创建输出目录失败: {e}"))?;

    // 从 manifest.txt 构建 filename → SHA1 映射
    let manifest = archive_files
        .iter()
        .find(|(name, _)| name == "manifest.txt")
        .ok_or("amproj 中未找到 manifest.txt")?;
    let manifest_str = String::from_utf8_lossy(&manifest.1);
    let sig_map = parse_manifest(&manifest_str);

    logger::info(&format!("manifest.txt 解析出 {} 条映射", sig_map.len()));
    for (filename, sha1) in &sig_map {
        logger::info(&format!("  {} → {}", filename, sha1));
    }

    let new_xml = rewrite_to_am_direct(&xml_content, &sig_map);
    let out_amproj = output_dir.join(format!("{}_fixed.amproj", proj_name));

    write_fixed_amproj(&out_amproj, &xml_name, &new_xml, &archive_files)?;

    logger::step(&format!("已生成修复 amproj: {}", out_amproj.display()));
    logger::info("URI 格式: amproj:filename → am:SHA1.ext（绕过导入重映射）");

    Ok(FixResult {
        output_amproj: out_amproj,
    })
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

/// 写入修复后的 amproj ZIP（保留原嵌入资源）
fn write_fixed_amproj(
    out_path: &Path,
    xml_name: &str,
    new_xml: &str,
    archive_files: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let out_file = std::fs::File::create(out_path).map_err(|e| format!("创建输出文件失败: {e}"))?;
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

/// 解析 manifest.txt，返回 {filename → SHA1} 映射
/// manifest 格式: `SHA1:filename` 或 `SHA1:proxyHash:filename`
fn parse_manifest(manifest: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in manifest.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, ':').collect();
        match parts.len() {
            2 => {
                // SHA1:filename → proxyHash 同 SHA1
                let sha1 = parts[0];
                let filename = parts[1];
                map.insert(filename.to_string(), sha1.to_string());
            }
            3 => {
                // SHA1:proxyHash:filename
                let proxy_hash = parts[1];
                let filename = parts[2];
                map.insert(filename.to_string(), proxy_hash.to_string());
            }
            _ => {}
        }
    }
    map
}

/// 将 XML 中所有 `amproj:filename` 替换为 `am:SHA1.ext`
fn rewrite_to_am_direct(xml: &str, sig_map: &std::collections::HashMap<String, String>) -> String {
    let re = Regex::new(r#"amproj:([^"]+)"#).unwrap();
    re.replace_all(xml, |caps: &regex::Captures| {
        let filename = &caps[1];
        if let Some(sha1) = sig_map.get(filename) {
            // 提取文件扩展名
            let ext = Path::new(filename)
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_default();
            if ext.is_empty() {
                format!("am:{sha1}")
            } else {
                format!("am:{sha1}.{ext}")
            }
        } else {
            // 没找到映射，保留原 URI
            logger::warn(&format!("manifest 中未找到文件: {filename}，保留原 URI"));
            format!("amproj:{filename}")
        }
    })
    .to_string()
}
