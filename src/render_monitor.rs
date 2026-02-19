// =============================================================================
// 渲染监控模块 — 轮询文件大小，严禁死等
// =============================================================================

use crate::adb;
use crate::config;
use crate::logger;
use crate::ui;

/// 监控渲染进度，返回完成后的视频文件手机端路径
pub fn wait_for_render_complete() -> Result<String, String> {
    logger::step("开始监控渲染状态");

    let mut elapsed: u64 = 0;

    // 阶段 1: 等待视频文件开始出现
    logger::info("等待视频文件生成...");
    let video_file = loop {
        if elapsed >= config::MAX_RENDER_WAIT_SECS {
            return Err(format!(
                "超过最大等待时间 {}s，未检测到视频文件",
                config::MAX_RENDER_WAIT_SECS
            ));
        }

        if let Ok(output) = adb::shell_cmd(&format!(
            "ls -t '{}'/*.mp4 2>/dev/null | head -1",
            config::PHONE_VIDEO_DIR
        )) {
            let file = output.trim().to_string();
            if !file.is_empty() && !file.contains("No such file") {
                logger::info(&format!("检测到视频文件: {file}"));
                break file;
            }
        }

        // 渲染等待期间也检查弹窗
        ui::dismiss_popups();

        std::thread::sleep(std::time::Duration::from_secs(config::POLL_INTERVAL_SECS));
        elapsed += config::POLL_INTERVAL_SECS;
        logger::info(&format!("  已等待 {elapsed}s，视频文件尚未生成..."));
    };

    // 阶段 2: 轮询文件大小直到稳定
    logger::info("开始轮询文件大小变化...");
    let stable_needed = config::STABLE_THRESHOLD_SECS / config::POLL_INTERVAL_SECS;
    let mut stable_count: u64 = 0;
    let mut last_size: i64 = -1;

    loop {
        if elapsed >= config::MAX_RENDER_WAIT_SECS {
            return Err(format!(
                "渲染监控超时（{}s），可能出现异常",
                config::MAX_RENDER_WAIT_SECS
            ));
        }

        let current_size = adb::shell_cmd(&format!("stat -c %s '{}' 2>/dev/null", video_file))
            .unwrap_or_default()
            .trim()
            .parse::<i64>()
            .unwrap_or(0);

        if current_size == last_size && current_size > 0 {
            stable_count += 1;
            logger::info(&format!(
                "  文件大小稳定: {current_size} bytes ({stable_count}/{stable_needed})"
            ));
        } else {
            if last_size >= 0 {
                logger::info(&format!(
                    "  文件大小变化: {last_size} -> {current_size} bytes"
                ));
            }
            stable_count = 0;
        }

        last_size = current_size;

        if stable_count >= stable_needed {
            // 检查是否还有临时文件
            let has_tmp = adb::shell_cmd(&format!(
                "ls '{}'/*.tmp '{}'/*.part 2>/dev/null",
                config::PHONE_VIDEO_DIR,
                config::PHONE_VIDEO_DIR
            ))
            .unwrap_or_default();

            if has_tmp.trim().is_empty() || has_tmp.contains("No such file") {
                logger::info(&format!("渲染完成！最终文件大小: {current_size} bytes"));
                return Ok(video_file);
            } else {
                logger::info("  仍有临时文件存在，继续等待...");
                stable_count = 0;
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(config::POLL_INTERVAL_SECS));
        elapsed += config::POLL_INTERVAL_SECS;
    }
}
