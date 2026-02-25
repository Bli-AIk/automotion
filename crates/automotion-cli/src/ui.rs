// =============================================================================
// UI 交互模块 — ADB 依赖的等待/点击/弹窗处理
// =============================================================================

use automotion_core::{config, logger};

use crate::adb;

pub use automotion_core::ui_parser::{find_element_by_id, find_element_by_text};

/// 等待并点击指定文本的 UI 元素（基于超时的轮询）
pub fn wait_and_tap_text(target_text: &str, timeout_secs: u64) -> bool {
    logger::info(&format!("等待 UI 元素出现: text=\"{target_text}\""));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    loop {
        if let Ok(xml) = adb::dump_ui_xml() {
            if let Some((cx, cy)) = find_element_by_text(&xml, target_text) {
                logger::info(&format!(
                    "找到元素 [{target_text}] 坐标: ({cx}, {cy})，执行点击"
                ));
                let _ = adb::tap(cx, cy);
                return true;
            }
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(
            config::UI_POLL_INTERVAL_MS,
        ));
    }

    logger::warn(&format!(
        "UI 元素 [{target_text}] 在 {timeout_secs}s 内未找到"
    ));
    false
}

/// 等待并点击指定 resource-id 的 UI 元素（基于超时的轮询）
pub fn wait_and_tap_by_resource_id(target_id: &str, timeout_secs: u64) -> bool {
    logger::info(&format!("等待 UI 元素出现: id=\"{target_id}\""));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    loop {
        if let Ok(xml) = adb::dump_ui_xml() {
            if let Some((cx, cy)) = find_element_by_id(&xml, target_id) {
                logger::info(&format!(
                    "找到元素 [{target_id}] 坐标: ({cx}, {cy})，执行点击"
                ));
                let _ = adb::tap(cx, cy);
                return true;
            }
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(
            config::UI_POLL_INTERVAL_MS,
        ));
    }

    logger::warn(&format!(
        "UI 元素 [{target_id}] 在 {timeout_secs}s 内未找到"
    ));
    false
}

/// 等待指定文本在 UI 中可见（不点击）
pub fn wait_for_text_visible(target_text: &str, timeout_secs: u64) -> bool {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    loop {
        if let Ok(xml) = adb::dump_ui_xml() {
            if find_element_by_text(&xml, target_text).is_some() {
                return true;
            }
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(
            config::UI_POLL_INTERVAL_MS,
        ));
    }
}

/// 尝试关闭可能出现的突发弹窗
pub fn dismiss_popups() {
    let xml = match adb::dump_ui_xml() {
        Ok(xml) => xml,
        Err(_) => return,
    };

    for text in config::POPUP_DISMISS_TEXTS {
        if let Some((cx, cy)) = find_element_by_text(&xml, text) {
            logger::warn(&format!("检测到弹窗 [{text}]，点击关闭: ({cx}, {cy})"));
            let _ = adb::tap(cx, cy);
            std::thread::sleep(std::time::Duration::from_secs(1));
            return;
        }
    }
}

/// 在 UI XML 中通过 content-desc 查找元素坐标
fn find_element_by_content_desc(xml: &str, target_desc: &str) -> Option<(i32, i32)> {
    // content-desc="完成" ... bounds="[x1,y1][x2,y2]"
    // 由于 automotion-cli 没有直接依赖 regex，用字符串查找
    let search = format!("content-desc=\"{}\"", target_desc);
    let desc_idx = xml.find(&search)?;
    let after_desc = &xml[desc_idx..];
    let bounds_prefix = "bounds=\"";
    let bounds_idx = after_desc.find(bounds_prefix)?;
    let bounds_start = bounds_idx + bounds_prefix.len();
    let bounds_end = after_desc[bounds_start..].find('"')?;
    let bounds_str = &after_desc[bounds_start..bounds_start + bounds_end];
    automotion_core::ui_parser::parse_bounds_center(bounds_str)
}

/// 在 Android 分享面板中点击"另存为"
/// 分享面板的图标较小，需要查找父容器的 bounds
pub fn tap_save_as_in_share_sheet(timeout_secs: u64) -> bool {
    logger::info("等待分享面板出现并点击「另存为」...");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    loop {
        if let Ok(xml) = adb::dump_ui_xml() {
            if let Some((cx, cy)) = find_element_by_text(&xml, "另存为") {
                // 文本位置可能太低（被截断），使用文本y坐标上方约100px作为点击位置
                let click_y = cy - 100;
                logger::info(&format!(
                    "找到「另存为」text=({cx},{cy}), 点击 ({cx},{click_y})"
                ));
                let _ = adb::tap(cx, click_y);
                return true;
            }
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(
            config::UI_POLL_INTERVAL_MS,
        ));
    }

    logger::warn("分享面板中未找到「另存为」按钮");
    false
}

/// 在 SAF 文件选择器中导航到 Download 并保存
/// 华为文件管理器的 SAF 流程：
///   1. 打开树形选择器（Download 通常已展开/选中）
///   2. 如果 Download 未选中则点击选中
///   3. 点击"完成"(content-desc="完成") 保存
pub fn save_in_saf_to_download(timeout_secs: u64) -> bool {
    logger::info("在 SAF 文件选择器中导航到 Download...");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

    // 等待 SAF 文件选择器出现
    // 华为 filemanager 的标志：content-desc="完成" 或 pick_path_selector 或 filemanager 包
    let mut saf_appeared = false;
    while std::time::Instant::now() < deadline {
        if let Ok(xml) = adb::dump_ui_xml() {
            if xml.contains("com.huawei.filemanager")
                || find_element_by_content_desc(&xml, "完成").is_some()
                || xml.contains("pick_path_selector")
            {
                saf_appeared = true;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(
            config::UI_POLL_INTERVAL_MS,
        ));
    }

    if !saf_appeared {
        logger::warn("SAF 文件选择器未出现");
        return false;
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

    // 检查 Download 是否已经选中（content-desc 包含 "已选中"）
    if let Ok(xml) = adb::dump_ui_xml() {
        let download_selected = xml.contains("Download") && xml.contains("已选中");
        if download_selected {
            logger::info("Download 已选中，直接点击「完成」");
        } else {
            // 尝试点击 Download 文件夹
            for scroll_attempt in 0..10 {
                if let Ok(xml2) = adb::dump_ui_xml() {
                    if let Some((cx, cy)) = find_element_by_text(&xml2, "Download") {
                        logger::info(&format!("找到 Download 文件夹: ({cx}, {cy})，点击选中"));
                        let _ = adb::tap(cx, cy);
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        break;
                    }
                }
                logger::info(&format!(
                    "  滚动查找 Download... (尝试 {}/10)",
                    scroll_attempt + 1
                ));
                let _ = adb::shell_cmd("input swipe 630 2000 630 1000 500");
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
        }
    }

    // 点击"完成"按钮
    if let Ok(xml) = adb::dump_ui_xml() {
        if let Some((dx, dy)) = find_element_by_content_desc(&xml, "完成") {
            logger::info(&format!("点击「完成」: ({dx}, {dy})"));
            let _ = adb::tap(dx, dy);
            std::thread::sleep(std::time::Duration::from_secs(2));
            return true;
        }
    }

    logger::warn("SAF 文件选择器中未找到「完成」按钮");
    false
}
