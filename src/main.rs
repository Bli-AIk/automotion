mod adb;
mod amproj;
mod config;
mod logger;
mod render_monitor;
mod ui;

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn main() {
    println!("============================================");
    println!("  automotion — Alemon 批量渲染自动化");
    println!("============================================");
    println!();

    // 信号陷阱：Ctrl+C 时恢复屏幕策略
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        logger::warn("收到中断信号，正在恢复设备状态...");
        let _ = adb::shell_cmd("svc power stayon false");
        let _ = adb::shell_cmd(&format!("am force-stop {}", config::PACKAGE_NAME));
        logger::info("屏幕常亮已关闭，Alemon 已停止。脚本退出。");
        r.store(false, Ordering::SeqCst);
        std::process::exit(1);
    })
    .expect("无法设置 Ctrl+C 处理器");

    // 环境初始化
    if let Err(e) = init_environment() {
        logger::error(&format!("环境初始化失败: {e}"));
        std::process::exit(1);
    }

    // 收集 .amproj 文件
    let mut projects: Vec<_> = fs::read_dir(config::LOCAL_INPUT_DIR)
        .expect(&format!("无法读取目录: {}", config::LOCAL_INPUT_DIR))
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "amproj")
        })
        .map(|e| e.path())
        .collect();
    projects.sort();

    let total = projects.len();
    if total == 0 {
        logger::warn(&format!(
            "在 {}/ 下未找到任何 .amproj 文件",
            config::LOCAL_INPUT_DIR
        ));
        restore_and_exit(0);
    }

    logger::info(&format!("共发现 {total} 个工程文件待处理"));
    println!();

    let mut success = 0u32;
    let mut failed = 0u32;

    for (i, proj_path) in projects.iter().enumerate() {
        if !running.load(Ordering::SeqCst) {
            break;
        }

        let idx = i + 1;
        logger::info(&format!("========== 进度: [{idx}/{total}] =========="));

        match process_single_project(proj_path) {
            Ok(()) => success += 1,
            Err(e) => {
                failed += 1;
                logger::error(&format!("工程处理失败: {e}，继续下一个..."));
            }
        }
    }

    println!();
    logger::step("==========================================");
    logger::step(&format!("全部处理完成"));
    logger::step(&format!("  成功: {success}/{total}"));
    logger::step(&format!("  失败: {failed}/{total}"));
    logger::step(&format!("  输出目录: {}/", config::LOCAL_OUTPUT_DIR));
    logger::step("==========================================");

    restore_and_exit(0);
}

fn init_environment() -> Result<(), String> {
    logger::step("初始化环境");

    // 检查 ADB 连接
    let serial = adb::get_serialno()?;
    logger::info(&format!("ADB 设备已连接: {serial}"));

    // 强制保持屏幕常亮
    adb::shell_cmd("svc power stayon true")?;
    logger::info("屏幕常亮已开启");

    // 确保本地输出目录存在
    fs::create_dir_all(config::LOCAL_OUTPUT_DIR)
        .map_err(|e| format!("创建输出目录失败: {e}"))?;

    // 确保手机端视频目录存在
    adb::shell_cmd(&format!("mkdir -p '{}'", config::PHONE_VIDEO_DIR))?;

    logger::info("环境初始化完成");
    Ok(())
}

fn process_single_project(proj_path: &std::path::Path) -> Result<(), String> {
    let proj_filename = proj_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let _proj_name = proj_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    logger::step("==========================================");
    logger::step(&format!("开始处理工程: {proj_filename}"));
    logger::step("==========================================");

    // 阶段 0: 分析 amproj 类型
    let (proj_type, proj_title) = amproj::analyze(proj_path)?;
    logger::info(&format!(
        "工程类型: {proj_type}, 内部标题: \"{proj_title}\""
    ));

    // 阶段 1: 推送文件到手机
    logger::step("[1/7] 推送工程文件");
    adb::push_file(
        proj_path.to_str().unwrap(),
        &format!("{}/{}", config::PHONE_DOWNLOAD_DIR, proj_filename),
    )?;

    // 阶段 2: 查询 MediaStore ID 并通过 content:// URI 触发导入
    logger::step("[2/7] 通过 content:// URI 触发导入");
    // 等待 MediaStore 索引（push 后可能需要一小段时间）
    std::thread::sleep(std::time::Duration::from_secs(2));
    let media_id = adb::query_media_id(&proj_filename)?;
    adb::open_project_via_content(&media_id)?;
    std::thread::sleep(std::time::Duration::from_secs(config::UI_LOAD_WAIT));

    // 阶段 3: 在导入确认对话框中点击"导入"
    logger::step("[3/7] 确认导入对话框");
    if !ui::wait_and_tap_text("导入", config::UI_RETRY_COUNT) {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err("无法找到导入确认按钮".into());
    }
    std::thread::sleep(std::time::Duration::from_secs(config::UI_LOAD_WAIT));

    // 阶段 4: 在导入完成对话框中点击"完成"
    logger::step("[4/7] 等待导入完成");
    if !ui::wait_and_tap_text("完成", config::UI_RETRY_COUNT) {
        logger::warn("未找到完成按钮，尝试关闭弹窗...");
        ui::dismiss_popups();
    }
    std::thread::sleep(std::time::Duration::from_secs(2));

    // 阶段 5: 启动 Alemon 并根据类型导航到对应标签页
    logger::step("[5/7] 打开 Alemon 并导航到对应标签页");
    adb::launch_app()?;
    std::thread::sleep(std::time::Duration::from_secs(config::UI_LOAD_WAIT));

    // 根据 amproj 类型导航到"项目"或"元素"标签
    let (tab_id, tab_text) = match proj_type {
        amproj::AmprojType::Project => ("tab_button_projects", "项目"),
        amproj::AmprojType::Element => ("tab_button_elements", "元素"),
    };
    logger::info(&format!("导航到「{tab_text}」标签"));

    if !ui::wait_and_tap_by_resource_id(tab_id, config::UI_RETRY_COUNT) {
        if !ui::wait_and_tap_text(tab_text, config::UI_RETRY_COUNT) {
            adb::force_stop();
            adb::cleanup_phone(&proj_filename, None);
            return Err(format!("无法导航到{tab_text}标签页"));
        }
    }
    std::thread::sleep(std::time::Duration::from_secs(3));

    // === 后续阶段：打开工程、导出、渲染监控、拉取 ===

    // 阶段 6: 在列表中找到刚导入的工程并打开
    logger::step("[6/9] 打开导入的工程");
    // amproj 的内部标题就是列表中显示的名称
    if !ui::wait_and_tap_text(&proj_title, config::UI_RETRY_COUNT) {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err(format!("在列表中未找到工程: {proj_title}"));
    }
    std::thread::sleep(std::time::Duration::from_secs(config::UI_LOAD_WAIT));

    // 阶段 7: 点击导出菜单并选择视频导出
    logger::step("[7/9] 打开导出菜单并选择视频");

    // 7a: 点击 share 按钮（右上角导出图标）
    if !ui::wait_and_tap_by_resource_id("share", config::UI_RETRY_COUNT) {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err("无法找到分享/导出按钮".into());
    }
    std::thread::sleep(std::time::Duration::from_secs(3));

    // 7b: 在导出选项列表中选择"视频"
    if !ui::wait_and_tap_text("视频", config::UI_RETRY_COUNT) {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err("无法找到视频导出选项".into());
    }
    std::thread::sleep(std::time::Duration::from_secs(3));

    // 7c: 在导出预览页面点击"保存"（saveButton）开始渲染
    if !ui::wait_and_tap_by_resource_id("saveButton", config::UI_RETRY_COUNT) {
        // 备用方案：通过文本查找
        if !ui::wait_and_tap_text("保存", config::UI_RETRY_COUNT) {
            adb::force_stop();
            adb::cleanup_phone(&proj_filename, None);
            return Err("无法找到保存按钮".into());
        }
    }

    // 阶段 8: 监控渲染完成
    logger::step("[8/9] 等待渲染完成");
    let video_remote_path = render_monitor::wait_for_render_complete(&proj_title)?;

    // 阶段 9: 拉取视频与清理
    logger::step("[9/9] 拉取视频并清理");
    let video_local_name = format!("{_proj_name}.mp4");
    adb::pull_file(
        &video_remote_path,
        &format!("{}/{}", config::LOCAL_OUTPUT_DIR, video_local_name),
    )?;

    adb::force_stop();
    adb::cleanup_phone(&proj_filename, Some(&video_remote_path));

    logger::step(&format!("工程 [{proj_filename}] 导入阶段完成 ✓"));
    println!();
    Ok(())
}

fn restore_and_exit(code: i32) -> ! {
    let _ = adb::shell_cmd("svc power stayon false");
    logger::info("屏幕常亮已恢复为默认策略");
    std::process::exit(code);
}
