// =============================================================================
// amproj 拆分模块 — 将大型 amproj 的顶级元素拆分为独立 amproj
// =============================================================================
//
// amproj 是 ZIP 格式，内含:
//   - UUID.xml: 项目 XML 数据
//   - manifest.txt: 资源文件清单
//   - 资源文件: PNG/MP3 等，以 HASH.ext 命名
//
// XML 结构:
//   <scene title="..." ...>
//     <media uri="amproj:HASH.ext" sig="HASH" ... />   ← 媒体资源声明
//     <shape id="..." label="..." ...>                   ← 内容元素
//     <embedScene id="..." label="编组 1" ...>           ← 编组（保持完整）
//       <scene ...>...</scene>
//     </embedScene>
//   </scene>
//
// 拆分策略:
//   1. 每个顶级非 <media> 元素单独成为一个 amproj
//   2. <embedScene>（编组）保持完整，含其所有子节点
//   3. 每个拆分 amproj 只包含该元素实际引用的资源文件

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::logger;

/// 拆分 amproj，返回输出文件路径列表
pub fn split_amproj(input: &Path, output_dir: &Path) -> Result<Vec<PathBuf>, String> {
    logger::step(&format!("开始拆分: {}", input.display()));

    let file =
        std::fs::File::open(input).map_err(|e| format!("无法打开文件: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("无法解析 ZIP: {e}"))?;

    // 读取 ZIP 中所有文件
    let mut archive_files: Vec<(String, Vec<u8>)> = Vec::new();
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
            xml_content = String::from_utf8_lossy(&data).to_string();
        }
        archive_files.push((name, data));
    }

    if xml_content.is_empty() {
        return Err("amproj 中未找到 XML 文件".into());
    }

    // 解析 XML
    let doc = roxmltree::Document::parse(&xml_content)
        .map_err(|e| format!("XML 解析失败: {e}"))?;
    let root = doc.root_element();

    if root.tag_name().name() != "scene" {
        return Err(format!(
            "根节点不是 <scene>，而是 <{}>",
            root.tag_name().name()
        ));
    }

    // 分离 <media> 声明和内容元素（跳过不可渲染的类型如 bookmark）
    let mut media_nodes = Vec::new();
    let mut content_nodes = Vec::new();
    let skip_tags = ["media", "bookmark"]; // bookmark 只是时间线标记，不可渲染

    for child in root.children().filter(|n| n.is_element()) {
        let tag = child.tag_name().name();
        if skip_tags.contains(&tag) {
            if tag == "media" {
                media_nodes.push(child);
            } else {
                logger::info(&format!("  跳过不可渲染元素: <{tag}>"));
            }
        } else {
            content_nodes.push(child);
        }
    }

    if content_nodes.is_empty() {
        logger::warn("scene 中没有内容元素可拆分");
        return Ok(Vec::new());
    }

    logger::info(&format!(
        "发现 {} 个顶级内容元素，{} 个媒体声明",
        content_nodes.len(),
        media_nodes.len()
    ));

    // 构建媒体信息索引
    let media_info: Vec<MediaInfo> = media_nodes
        .iter()
        .map(|m| {
            let sig = m.attribute("sig").unwrap_or("").to_string();
            let uri = m.attribute("uri").unwrap_or("").to_string();
            let asset_filename = uri
                .strip_prefix("amproj:")
                .unwrap_or("")
                .to_string();
            MediaInfo {
                sig,
                asset_filename,
                node: *m,
            }
        })
        .collect();

    // 创建输出目录
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("创建输出目录失败: {e}"))?;

    let mut outputs = Vec::new();

    for (i, child) in content_nodes.iter().enumerate() {
        let label = child
            .attribute("label")
            .or(child.attribute("id"))
            .unwrap_or("element");
        let tag = child.tag_name().name();

        // 递归收集子树中所有资源引用
        let refs = collect_resource_refs(child);

        // 确定需要的 media 元素
        let needed_media: Vec<&MediaInfo> = media_info
            .iter()
            .filter(|m| refs.contains(&m.sig) || refs.contains(&m.asset_filename))
            .collect();

        // 提取 XML 片段
        let child_xml = &xml_content[child.range()];
        let media_xmls: Vec<&str> = needed_media
            .iter()
            .map(|m| &xml_content[m.node.range()])
            .collect();

        // 根据子元素的 endTime 调整 totalTime
        let end_time = child.attribute("endTime");

        // 构建新的 scene XML
        let root_attrs: Vec<(&str, &str)> = root
            .attributes()
            .map(|a| (a.name(), a.value()))
            .collect();
        let new_xml = build_scene_xml(&root_attrs, label, end_time, &media_xmls, child_xml);

        // 收集需要的资源文件名
        let needed_assets: HashSet<&str> = needed_media
            .iter()
            .filter(|m| !m.asset_filename.is_empty())
            .map(|m| m.asset_filename.as_str())
            .collect();

        // 生成输出文件名
        let safe_label = sanitize_filename(label);
        let out_name = format!("{:03}_{safe_label}.amproj", i);
        let out_path = output_dir.join(&out_name);

        // 创建 ZIP
        write_split_amproj(&out_path, i, &safe_label, &new_xml, &archive_files, &needed_assets)?;

        logger::info(&format!(
            "  [{i}] {out_name}  ({tag}, label={label:?}, 资源: {}个)",
            needed_assets.len()
        ));

        outputs.push(out_path);
    }

    logger::step(&format!(
        "拆分完成: {} 个元素输出到 {}",
        outputs.len(),
        output_dir.display()
    ));
    Ok(outputs)
}

struct MediaInfo<'a> {
    sig: String,
    asset_filename: String,
    node: roxmltree::Node<'a, 'a>,
}

/// 递归收集节点子树中所有资源引用（sig 和 asset filename）
fn collect_resource_refs(node: &roxmltree::Node) -> HashSet<String> {
    let mut refs = HashSet::new();
    collect_refs_recursive(node, &mut refs);
    refs
}

fn collect_refs_recursive(node: &roxmltree::Node, refs: &mut HashSet<String>) {
    for attr in node.attributes() {
        let val = attr.value();
        // amproj: 前缀的资源引用
        if let Some(filename) = val.strip_prefix("amproj:") {
            refs.insert(filename.to_string());
            // 同时提取不带扩展名的 sig
            if let Some((sig, _ext)) = filename.rsplit_once('.') {
                refs.insert(sig.to_string());
            }
        }
        // sig 属性直接就是资源哈希
        if attr.name() == "sig" && !val.is_empty() {
            refs.insert(val.to_string());
        }
    }
    for child in node.children().filter(|n| n.is_element()) {
        collect_refs_recursive(&child, refs);
    }
}

/// 重建 scene XML
fn build_scene_xml(
    root_attrs: &[(&str, &str)],
    title: &str,
    total_time: Option<&str>,
    media_xmls: &[&str],
    child_xml: &str,
) -> String {
    let mut xml = String::from("<?xml version='1.0' encoding='UTF-8' ?>\n<scene");

    for &(name, value) in root_attrs {
        let val = match name {
            "title" => title,
            "totalTime" => total_time.unwrap_or(value),
            _ => value,
        };
        xml.push_str(&format!(" {}=\"{}\"", name, escape_xml_attr(val)));
    }
    xml.push_str(">\n");

    for media in media_xmls {
        xml.push_str("  ");
        xml.push_str(media);
        xml.push('\n');
    }

    xml.push_str("  ");
    xml.push_str(child_xml);
    xml.push('\n');

    xml.push_str("</scene>\n");
    xml
}

/// 写入拆分后的 amproj ZIP
fn write_split_amproj(
    out_path: &Path,
    index: usize,
    safe_label: &str,
    xml_content: &str,
    archive_files: &[(String, Vec<u8>)],
    needed_assets: &HashSet<&str>,
) -> Result<(), String> {
    let out_file =
        std::fs::File::create(out_path).map_err(|e| format!("创建输出文件失败: {e}"))?;
    let mut zip_writer = zip::ZipWriter::new(out_file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 写入 XML（使用索引+标签命名）
    let xml_filename = format!("{:03}_{safe_label}.xml", index);
    zip_writer
        .start_file(&xml_filename, options)
        .map_err(|e| format!("ZIP 写入失败: {e}"))?;
    std::io::Write::write_all(&mut zip_writer, xml_content.as_bytes())
        .map_err(|e| format!("写入 XML 失败: {e}"))?;

    // 写入需要的资源文件
    let mut manifest_lines = Vec::new();
    for (name, data) in archive_files {
        if needed_assets.contains(name.as_str()) {
            zip_writer
                .start_file(name, options)
                .map_err(|e| format!("ZIP 写入资源失败: {e}"))?;
            std::io::Write::write_all(&mut zip_writer, data)
                .map_err(|e| format!("写入资源失败: {e}"))?;
            manifest_lines.push(name.clone());
        }
    }

    // 写入 manifest.txt
    zip_writer
        .start_file("manifest.txt", options)
        .map_err(|e| format!("ZIP 写入 manifest 失败: {e}"))?;
    let manifest = manifest_lines.join("\n");
    std::io::Write::write_all(&mut zip_writer, manifest.as_bytes())
        .map_err(|e| format!("写入 manifest 失败: {e}"))?;

    zip_writer
        .finish()
        .map_err(|e| format!("ZIP 完成失败: {e}"))?;

    Ok(())
}

fn escape_xml_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn sanitize_filename(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect();
    cleaned.trim().replace(' ', "_")
}
