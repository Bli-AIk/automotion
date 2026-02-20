// =============================================================================
// UI XML 解析模块 — 纯函数，无 ADB 依赖（可跨平台复用）
// =============================================================================

use regex::Regex;

/// 从 bounds 字符串 "[x1,y1][x2,y2]" 解析中心坐标
pub fn parse_bounds_center(bounds: &str) -> Option<(i32, i32)> {
    let re = Regex::new(r"\[(\d+),(\d+)\]\[(\d+),(\d+)\]").unwrap();
    let caps = re.captures(bounds)?;

    let x1: i32 = caps[1].parse().ok()?;
    let y1: i32 = caps[2].parse().ok()?;
    let x2: i32 = caps[3].parse().ok()?;
    let y2: i32 = caps[4].parse().ok()?;

    Some(((x1 + x2) / 2, (y1 + y2) / 2))
}

/// 在 UI XML 中查找包含指定文本的节点，返回其中心坐标
pub fn find_element_by_text(xml: &str, target_text: &str) -> Option<(i32, i32)> {
    let pattern = format!(
        r#"text="[^"]*{}[^"]*"[^>]*bounds="(\[[0-9]+,[0-9]+\]\[[0-9]+,[0-9]+\])""#,
        regex::escape(target_text)
    );
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(xml)?;
    parse_bounds_center(&caps[1])
}

/// 在 UI XML 中查找包含指定 resource-id 的节点，返回其中心坐标
pub fn find_element_by_id(xml: &str, target_id: &str) -> Option<(i32, i32)> {
    let pattern = format!(
        r#"resource-id="[^"]*{}[^"]*"[^>]*bounds="(\[[0-9]+,[0-9]+\]\[[0-9]+,[0-9]+\])""#,
        regex::escape(target_id)
    );
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(xml)?;
    parse_bounds_center(&caps[1])
}
