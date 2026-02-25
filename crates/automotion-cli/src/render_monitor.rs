// =============================================================================
// 渲染监控模块 — 轮询文件大小，严禁死等
// =============================================================================

use automotion_core::{config, logger};

use crate::adb;

/// 监控渲染进度，返回完成后的视频文件手机端路径
/// `proj_title` 是 amproj 内部的工程标题，用于匹配输出文件名
/// Alemon 输出的视频文件命名格式: "{标题} [{哈希}].mp4"
pub fn wait_for_render_complete(proj_title: &str) -> Result<String, String> {
    logger::step("开始监控渲染状态");

    let mut elapsed: u64 = 0;

    // 阶段 1: 等待匹配的视频文件出现
    logger::info(&format!(
        "等待视频文件生成（匹配标题: \"{proj_title}\"）..."
    ));
    let video_file = loop {
        if elapsed >= config::MAX_RENDER_WAIT_SECS {
            return Err(format!(
                "超过最大等待时间 {}s，未检测到视频文件",
                config::MAX_RENDER_WAIT_SECS
            ));
        }

        // 列出目录中所有 mp4 文件，按时间倒序，找匹配标题的最新文件
        if let Ok(output) = adb::shell_cmd(&format!(
            "ls -t '{}'/*.mp4 2>/dev/null",
            config::PHONE_VIDEO_DIR
        )) {
            for line in output.lines() {
                let filename = line.trim();
                if !filename.is_empty()
                    && !filename.contains("No such file")
                    && filename.contains(proj_title)
                {
                    logger::info(&format!("检测到匹配视频文件: {filename}"));
                    break; // 外层 loop 需要值
                }
            }
            // 重新查找返回值
            let found = output.lines().find(|l| {
                let f = l.trim();
                !f.is_empty() && !f.contains("No such file") && f.contains(proj_title)
            });
            if let Some(f) = found {
                let file = f.trim().to_string();
                logger::info(&format!("检测到匹配视频文件: {file}"));
                break file;
            }
        }

        // 等待视频文件出现
        std::thread::sleep(std::time::Duration::from_secs(config::POLL_INTERVAL_SECS));
        elapsed += config::POLL_INTERVAL_SECS;
        if elapsed % 15 == 0 {
            logger::info(&format!("  已等待 {elapsed}s，视频文件尚未生成..."));
        }
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
