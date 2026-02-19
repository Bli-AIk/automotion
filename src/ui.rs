// =============================================================================
// UI 坐标解析与交互模块（核心：严禁写死坐标）
// =============================================================================

use regex::Regex;

use crate::adb;
use crate::config;
use crate::logger;

/// 从 bounds 字符串 "[x1,y1][x2,y2]" 解析中心坐标
fn parse_bounds_center(bounds: &str) -> Option<(i32, i32)> {
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
    // 匹配 text="...目标文本..." 且同一节点中有 bounds="[x1,y1][x2,y2]"
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

/// 等待并点击指定文本的 UI 元素（带重试）
pub fn wait_and_tap_text(target_text: &str, max_retries: u32) -> bool {
    logger::info(&format!("等待 UI 元素出现: text=\"{target_text}\""));

    for retry in 1..=max_retries {
        if let Ok(xml) = adb::dump_ui_xml() {
            if let Some((cx, cy)) = find_element_by_text(&xml, target_text) {
                logger::info(&format!(
                    "找到元素 [{target_text}] 坐标: ({cx}, {cy})，执行点击"
                ));
                let _ = adb::tap(cx, cy);
                return true;
            }
        }
        logger::info(&format!(
            "  第 {retry}/{max_retries} 次重试，等待 {}s...",
            config::UI_LOAD_WAIT
        ));
        std::thread::sleep(std::time::Duration::from_secs(config::UI_LOAD_WAIT));
    }

    logger::warn(&format!(
        "UI 元素 [{target_text}] 在 {max_retries} 次尝试后未找到"
    ));
    false
}

/// 等待并点击指定 resource-id 的 UI 元素（带重试）
pub fn wait_and_tap_by_resource_id(target_id: &str, max_retries: u32) -> bool {
    logger::info(&format!("等待 UI 元素出现: id=\"{target_id}\""));

    for retry in 1..=max_retries {
        if let Ok(xml) = adb::dump_ui_xml() {
            if let Some((cx, cy)) = find_element_by_id(&xml, target_id) {
                logger::info(&format!(
                    "找到元素 [{target_id}] 坐标: ({cx}, {cy})，执行点击"
                ));
                let _ = adb::tap(cx, cy);
                return true;
            }
        }
        logger::info(&format!(
            "  第 {retry}/{max_retries} 次重试，等待 {}s...",
            config::UI_LOAD_WAIT
        ));
        std::thread::sleep(std::time::Duration::from_secs(config::UI_LOAD_WAIT));
    }

    logger::warn(&format!(
        "UI 元素 [{target_id}] 在 {max_retries} 次尝试后未找到"
    ));
    false
}

/// 尝试关闭可能出现的突发弹窗
pub fn dismiss_popups() {
    let xml = match adb::dump_ui_xml() {
        Ok(xml) => xml,
        Err(_) => return,
    };

    for text in config::POPUP_DISMISS_TEXTS {
        if let Some((cx, cy)) = find_element_by_text(&xml, text) {
            logger::warn(&format!(
                "检测到弹窗 [{text}]，点击关闭: ({cx}, {cy})"
            ));
            let _ = adb::tap(cx, cy);
            std::thread::sleep(std::time::Duration::from_secs(1));
            return;
        }
    }
}
