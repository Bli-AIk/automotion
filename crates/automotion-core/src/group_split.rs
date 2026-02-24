// =============================================================================
// amproj 编组+时间切分模块 — 将 amproj 全部元素编组后按帧数切分
// =============================================================================
//
// 工作流程:
//   1. 编组: 将所有顶级内容元素移入一个 embedScene 编组
//   2. 切分: 将编组工程按指定帧数拆分为若干独立 amproj
//
// 编组后的 XML 结构:
//   <scene ...>
//     <media ... />                              ← 保留在外层
//     <bookmark ... />                           ← 保留在外层
//     <embedScene id="..." label="编组" ...>     ← 新建的编组包装器
//       <transform><location value="0,0,0" /></transform>
//       <fillColor value="#ff000000" />
//       <scene ...>                              ← 内层场景（含所有原始内容元素）
//         <audio ... />
//         <shape ... />
//         <embedScene ... />
//         ...
//       </scene>
//     </embedScene>
//   </scene>
//
// 时间切分原理:
//   - 内层 scene 内容完全不变
//   - 通过调整外层 scene totalTime 和 embedScene 的 inTime/outTime 控制时间窗口
//   - outTime = i32::MAX - inner_totalTime + chunk_duration + inTime

use std::io::Read;
use std::path::{Path, PathBuf};

use crate::logger;

/// 默认每段帧数
pub const DEFAULT_FRAMES_PER_CHUNK: u32 = 30;

/// 编组并按帧切分 amproj，返回输出文件路径列表
pub fn group_and_split(
    input: &Path,
    output_dir: &Path,
    frames_per_chunk: u32,
) -> Result<Vec<PathBuf>, String> {
    logger::step(&format!("开始编组+切分: {}", input.display()));

    let (xml_content, archive_files) = read_amproj_zip(input)?;

    let doc = roxmltree::Document::parse(&xml_content)
        .map_err(|e| format!("XML 解析失败: {e}"))?;
    let root = doc.root_element();

    if root.tag_name().name() != "scene" {
        return Err(format!(
            "根节点不是 <scene>，而是 <{}>",
            root.tag_name().name()
        ));
    }

    // 提取场景属性
    let total_time: u64 = root
        .attribute("totalTime")
        .ok_or("scene 缺少 totalTime 属性")?
        .parse()
        .map_err(|e| format!("totalTime 解析失败: {e}"))?;
    let fps: u32 = root
        .attribute("fps")
        .ok_or("scene 缺少 fps 属性")?
        .parse()
        .map_err(|e| format!("fps 解析失败: {e}"))?;
    let title = root.attribute("title").unwrap_or("未知");

    logger::info(&format!(
        "场景信息: title=\"{title}\", totalTime={total_time}, fps={fps}"
    ));

    // 阶段 1: 编组
    let grouped_xml = build_grouped_xml(&xml_content, &root, total_time)?;

    // 阶段 2: 按帧切分
    let chunk_duration = (frames_per_chunk + 1) as u64 * 1000 / fps as u64 - 1;
    let total_frames = (total_time as f64 * fps as f64 / 1000.0).ceil() as u64;
    let chunk_count =
        (total_frames + frames_per_chunk as u64 - 1) / frames_per_chunk as u64;

    logger::info(&format!(
        "切分参数: 每段{frames_per_chunk}帧, 段时长={chunk_duration}ms, 总帧数≈{total_frames}, 段数={chunk_count}"
    ));

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("创建输出目录失败: {e}"))?;

    let mut outputs = Vec::new();

    for chunk_idx in 0..chunk_count {
        let in_time = chunk_idx * frames_per_chunk as u64 * 1000 / fps as u64;

        // 最后一个切片的持续时间可能不足完整的 chunk_duration
        let remaining_time = total_time.saturating_sub(in_time);
        let actual_duration = chunk_duration.min(remaining_time);

        let out_time: i64 =
            i32::MAX as i64 - total_time as i64 + actual_duration as i64 + in_time as i64;

        let split_xml =
            build_split_xml(&grouped_xml, actual_duration, in_time, out_time, chunk_idx, frames_per_chunk);

        let start_frame = chunk_idx * frames_per_chunk as u64;
        let end_frame = ((chunk_idx + 1) * frames_per_chunk as u64).min(total_frames);
        let out_name = format!("{start_frame}-{end_frame}.amproj");
        let out_path = output_dir.join(&out_name);

        write_amproj_zip(&out_path, &split_xml, &archive_files)?;

        logger::info(&format!(
            "  [{chunk_idx}] {out_name}  (帧 {start_frame}-{end_frame}, inTime={in_time}, duration={actual_duration})"
        ));

        outputs.push(out_path);
    }

    logger::step(&format!(
        "编组+切分完成: {} 个片段输出到 {}",
        outputs.len(),
        output_dir.display()
    ));
    Ok(outputs)
}

/// 仅执行编组操作，输出单个编组后的 amproj
pub fn group_amproj(input: &Path, output_dir: &Path) -> Result<PathBuf, String> {
    logger::step(&format!("开始编组: {}", input.display()));

    let (xml_content, archive_files) = read_amproj_zip(input)?;

    let doc = roxmltree::Document::parse(&xml_content)
        .map_err(|e| format!("XML 解析失败: {e}"))?;
    let root = doc.root_element();

    if root.tag_name().name() != "scene" {
        return Err(format!(
            "根节点不是 <scene>，而是 <{}>",
            root.tag_name().name()
        ));
    }

    let total_time: u64 = root
        .attribute("totalTime")
        .ok_or("scene 缺少 totalTime 属性")?
        .parse()
        .map_err(|e| format!("totalTime 解析失败: {e}"))?;

    let grouped_xml = build_grouped_xml(&xml_content, &root, total_time)?;

    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("创建输出目录失败: {e}"))?;

    let title = root.attribute("title").unwrap_or("未知");
    let safe_title = sanitize_filename(title);
    let out_name = format!("{safe_title}_grouped.amproj");
    let out_path = output_dir.join(&out_name);

    write_amproj_zip(&out_path, &grouped_xml, &archive_files)?;

    logger::step(&format!("编组完成: {}", out_path.display()));
    Ok(out_path)
}

// =============================================================================
// 内部辅助函数
// =============================================================================

/// 读取 amproj ZIP，返回 (XML 内容, 所有文件列表)
fn read_amproj_zip(input: &Path) -> Result<(String, Vec<(String, Vec<u8>)>), String> {
    let file =
        std::fs::File::open(input).map_err(|e| format!("无法打开文件: {e}"))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("无法解析 ZIP: {e}"))?;

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

    Ok((xml_content, archive_files))
}

/// 构建编组后的 XML
fn build_grouped_xml(
    xml_content: &str,
    root: &roxmltree::Node,
    total_time: u64,
) -> Result<String, String> {
    let keep_tags = ["media", "bookmark"];

    // 收集保留在外层的元素和需要移入编组的元素
    let mut kept_xmls = Vec::new();
    let mut content_xmls = Vec::new();

    for child in root.children().filter(|n| n.is_element()) {
        let tag = child.tag_name().name();
        let fragment = &xml_content[child.range()];
        if keep_tags.contains(&tag) {
            kept_xmls.push(fragment);
        } else {
            content_xmls.push(fragment);
        }
    }

    if content_xmls.is_empty() {
        return Err("scene 中没有内容元素可编组".into());
    }

    logger::info(&format!(
        "编组: {} 个外层元素保留, {} 个内容元素移入编组",
        kept_xmls.len(),
        content_xmls.len()
    ));

    // 提取场景属性
    let width = root.attribute("width").unwrap_or("1280");
    let height = root.attribute("height").unwrap_or("960");
    let fps = root.attribute("fps").unwrap_or("60");

    // 收集 root scene 的所有属性用于复刻内层 scene
    let _inner_scene_attrs = build_inner_scene_attrs(root);

    // 生成编组 ID（使用简单递增策略）
    let group_id = 99830094u64;

    // 构建完整的编组后 XML
    let mut xml = String::new();

    // XML 声明
    xml.push_str("<?xml version='1.0' encoding='UTF-8' ?>\n");

    // 外层 scene 开标签
    xml.push_str("<scene");
    for attr in root.attributes() {
        let val = escape_xml_attr(attr.value());
        xml.push_str(&format!(" {}=\"{}\"", attr.name(), val));
    }
    xml.push_str(">\n");

    // 外层保留的元素（media, bookmark）
    for fragment in &kept_xmls {
        xml.push_str("  ");
        xml.push_str(fragment);
        xml.push('\n');
    }

    // embedScene 编组包装器
    xml.push_str(&format!(
        "  <embedScene id=\"{group_id}\" label=\"编组\" startTime=\"0\" endTime=\"{total_time}\" fillType=\"intrinsic\">\n"
    ));

    // transform — 位置设为画布中心，因为 embedScene 锚点默认在内层 scene 中心
    let cx: f64 = width.parse::<f64>().unwrap_or(1280.0) / 2.0;
    let cy: f64 = height.parse::<f64>().unwrap_or(960.0) / 2.0;
    xml.push_str("    <transform>\n");
    xml.push_str(&format!(
        "      <location value=\"{:.6},{:.6},0.000000\" />\n",
        cx, cy
    ));
    xml.push_str("    </transform>\n");

    // fillColor
    xml.push_str("    <fillColor value=\"#ff000000\" />\n");

    // 内层 scene
    xml.push_str(&format!(
        "    <scene title=\"\" width=\"{width}\" height=\"{height}\" exportWidth=\"{width}\" exportHeight=\"{height}\" precompose=\"dynamicResolution\" bgcolor=\"#00000000\" totalTime=\"{total_time}\" fps=\"{fps}\" modifiedTime=\"0\" amver=\"{amver}\" ffver=\"{ffver}\" am=\"{am}\" amplatform=\"{amplatform}\" retime=\"off\" retimeAdaptFPS=\"false\">\n",
        amver = root.attribute("amver").unwrap_or("43475"),
        ffver = root.attribute("ffver").unwrap_or("105"),
        am = escape_xml_attr(root.attribute("am").unwrap_or("com.taffy.alemon/4.3.4.3019")),
        amplatform = root.attribute("amplatform").unwrap_or("android"),
    ));

    // 内容元素
    for fragment in &content_xmls {
        xml.push_str("      ");
        xml.push_str(fragment);
        xml.push('\n');
    }

    xml.push_str("    </scene>\n");
    xml.push_str("  </embedScene>\n");
    xml.push_str("</scene>\n");

    Ok(xml)
}

/// 构建时间切分后的 XML
fn build_split_xml(
    grouped_xml: &str,
    chunk_duration: u64,
    in_time: u64,
    out_time: i64,
    _chunk_idx: u64,
    _frames_per_chunk: u32,
) -> String {
    let mut result = grouped_xml.to_string();

    // 替换外层 scene 的 totalTime
    result = replace_scene_total_time(&result, chunk_duration);

    // 替换 embedScene 的 endTime 并添加 inTime/outTime
    result = replace_embed_scene_time(&result, chunk_duration, in_time, out_time);

    result
}

/// 替换外层 scene 的 totalTime 属性
fn replace_scene_total_time(xml: &str, new_total_time: u64) -> String {
    // 找到第一个 <scene 标签中的 totalTime 属性并替换
    if let Some(scene_start) = xml.find("<scene") {
        // 找到这个 scene 标签的结束位置
        if let Some(tag_end) = xml[scene_start..].find('>') {
            let tag = &xml[scene_start..scene_start + tag_end + 1];
            // 替换 totalTime
            if let Some(tt_start) = tag.find("totalTime=\"") {
                let attr_start = scene_start + tt_start + "totalTime=\"".len();
                if let Some(attr_end) = xml[attr_start..].find('"') {
                    let mut new_xml = String::new();
                    new_xml.push_str(&xml[..attr_start]);
                    new_xml.push_str(&new_total_time.to_string());
                    new_xml.push_str(&xml[attr_start + attr_end..]);
                    return new_xml;
                }
            }
        }
    }
    xml.to_string()
}

/// 替换 embedScene 的时间属性
fn replace_embed_scene_time(xml: &str, end_time: u64, in_time: u64, out_time: i64) -> String {
    if let Some(es_start) = xml.find("<embedScene") {
        if let Some(tag_end) = xml[es_start..].find('>') {
            let old_tag = &xml[es_start..es_start + tag_end];

            // 逐个替换/添加属性
            let mut tag_str = old_tag.to_string();

            // 替换 endTime
            if let Some(pos) = tag_str.find("endTime=\"") {
                let attr_start = pos + "endTime=\"".len();
                if let Some(attr_end) = tag_str[attr_start..].find('"') {
                    tag_str = format!(
                        "{}{}{}",
                        &tag_str[..attr_start],
                        end_time,
                        &tag_str[attr_start + attr_end..]
                    );
                }
            }

            // 添加 inTime（如果 > 0）
            if in_time > 0 {
                // 在 fillType="intrinsic" 后添加
                if let Some(pos) = tag_str.find("fillType=\"intrinsic\"") {
                    let insert_pos = pos + "fillType=\"intrinsic\"".len();
                    tag_str = format!(
                        "{} inTime=\"{}\"{}",
                        &tag_str[..insert_pos],
                        in_time,
                        &tag_str[insert_pos..]
                    );
                }
            }

            // 添加 outTime
            if let Some(pos) = tag_str.find("fillType=\"intrinsic\"") {
                let insert_pos = if in_time > 0 {
                    // inTime 已经插入在 fillType 后，找到 inTime 后的位置
                    if let Some(it_pos) = tag_str.find(&format!("inTime=\"{}\"", in_time)) {
                        it_pos + format!("inTime=\"{}\"", in_time).len()
                    } else {
                        pos + "fillType=\"intrinsic\"".len()
                    }
                } else {
                    pos + "fillType=\"intrinsic\"".len()
                };
                tag_str = format!(
                    "{} outTime=\"{}\"{}",
                    &tag_str[..insert_pos],
                    out_time,
                    &tag_str[insert_pos..]
                );
            }

            let mut result = String::new();
            result.push_str(&xml[..es_start]);
            result.push_str(&tag_str);
            result.push_str(&xml[es_start + tag_end..]);
            return result;
        }
    }
    xml.to_string()
}

/// 写入 amproj ZIP（保留所有原始资源文件，XML 使用新内容）
fn write_amproj_zip(
    out_path: &Path,
    xml_content: &str,
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

        if name.ends_with(".xml") {
            // 写入修改后的 XML
            std::io::Write::write_all(&mut zip_writer, xml_content.as_bytes())
                .map_err(|e| format!("写入 XML 失败: {e}"))?;
        } else {
            // 原样保留其他文件
            std::io::Write::write_all(&mut zip_writer, data)
                .map_err(|e| format!("写入资源失败: {e}"))?;
        }
    }

    zip_writer
        .finish()
        .map_err(|e| format!("ZIP 完成失败: {e}"))?;

    Ok(())
}

/// 从 root 场景属性构建内层场景属性字符串（辅助函数，未使用）
fn build_inner_scene_attrs(root: &roxmltree::Node) -> Vec<(String, String)> {
    root.attributes()
        .map(|a| (a.name().to_string(), a.value().to_string()))
        .collect()
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
