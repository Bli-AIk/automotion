// =============================================================================
// UI 交互模块 — ADB 依赖的等待/点击/弹窗处理
// =============================================================================

use automotion_core::{config, logger};

use crate::adb;

pub use automotion_core::ui_parser::{find_element_by_id, find_element_by_text};

/// 等待并点击指定文本的 UI 元素（基于超时的轮询）
pub fn wait_and_tap_text(target_text: &str, timeout_secs: u64) -> bool {
    logger::info(&format!("等待 UI 元素出现: text=\"{target_text}\""));
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs(timeout_secs);

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
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs(timeout_secs);

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
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs(timeout_secs);

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
            logger::warn(&format!(
                "检测到弹窗 [{text}]，点击关闭: ({cx}, {cy})"
            ));
            let _ = adb::tap(cx, cy);
            std::thread::sleep(std::time::Duration::from_secs(1));
            return;
        }
    }
}
