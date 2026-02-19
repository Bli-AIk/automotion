mod adb;
mod render_monitor;
mod ui;

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clap::{Parser, Subcommand};

use automotion_core::{amproj, config, logger, split};

#[derive(Parser)]
#[command(name = "automotion", about = "Alemon 批量渲染自动化工具")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// 批量渲染 amproj 为 MP4 视频（默认模式）
    Render {
        /// 输入目录
        #[arg(short, long, default_value = "./input_projects")]
        input: String,
        /// 输出目录
        #[arg(short, long, default_value = "./output_videos")]
        output: String,
    },
    /// 拆分大型 amproj 为独立元素
    Split {
        /// 要拆分的 amproj 文件路径
        file: String,
        /// 输出目录
        #[arg(short, long, default_value = "./output_split")]
        output: String,
    },
    /// 拆分 amproj 后直接批量渲染为 MP4
    SplitRender {
        /// 要拆分的 amproj 文件路径
        file: String,
        /// 视频输出目录
        #[arg(short, long, default_value = "./output_videos")]
        output: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Commands::Render { .. }) => {
            // 默认模式 / render 子命令
            let (input, output) = match cli.command {
                Some(Commands::Render { input, output }) => (input, output),
                _ => (
                    "./input_projects".to_string(),
                    "./output_videos".to_string(),
                ),
            };
            run_render(&input, &output);
        }
        Some(Commands::Split { file, output }) => {
            run_split(&file, &output);
        }
        Some(Commands::SplitRender { file, output }) => {
            run_split_render(&file, &output);
        }
    }
}

// =============================================================================
// 子命令实现
// =============================================================================

/// render: 批量渲染 input 目录中的 amproj 文件
fn run_render(input_dir: &str, output_dir: &str) {
    println!("============================================");
    println!("  automotion — Alemon 批量渲染自动化");
    println!("============================================");
    println!();

    let running = setup_signal_handler();

    if let Err(e) = init_environment(output_dir) {
        logger::error(&format!("环境初始化失败: {e}"));
        std::process::exit(1);
    }

    let projects = collect_amproj_files(input_dir);
    if projects.is_empty() {
        logger::warn(&format!("在 {input_dir}/ 下未找到任何 .amproj 文件"));
        restore_and_exit(0);
    }

    batch_render(&projects, output_dir, &running);
    restore_and_exit(0);
}

/// split: 拆分单个 amproj 为多个独立元素
fn run_split(file: &str, output_dir: &str) {
    println!("============================================");
    println!("  automotion — amproj 拆分工具");
    println!("============================================");
    println!();

    let input_path = PathBuf::from(file);
    if !input_path.exists() {
        logger::error(&format!("文件不存在: {file}"));
        std::process::exit(1);
    }

    let output_path = PathBuf::from(output_dir);
    match split::split_amproj(&input_path, &output_path) {
        Ok(outputs) => {
            logger::step(&format!("拆分完成，共 {} 个文件", outputs.len()));
        }
        Err(e) => {
            logger::error(&format!("拆分失败: {e}"));
            std::process::exit(1);
        }
    }
}

/// split-render: 先拆分再批量渲染
fn run_split_render(file: &str, output_dir: &str) {
    println!("============================================");
    println!("  automotion — 拆分 + 批量渲染");
    println!("============================================");
    println!();

    let input_path = PathBuf::from(file);
    if !input_path.exists() {
        logger::error(&format!("文件不存在: {file}"));
        std::process::exit(1);
    }

    // 阶段 1: 拆分到临时目录
    let split_dir = PathBuf::from("./.automotion_split_tmp");
    let split_outputs = match split::split_amproj(&input_path, &split_dir) {
        Ok(o) => o,
        Err(e) => {
            logger::error(&format!("拆分失败: {e}"));
            std::process::exit(1);
        }
    };

    if split_outputs.is_empty() {
        logger::warn("拆分结果为空，无文件可渲染");
        std::process::exit(0);
    }

    // 阶段 2: 批量渲染
    let running = setup_signal_handler();

    if let Err(e) = init_environment(output_dir) {
        logger::error(&format!("环境初始化失败: {e}"));
        std::process::exit(1);
    }

    batch_render(&split_outputs, output_dir, &running);

    // 清理临时拆分目录
    if split_dir.exists() {
        let _ = fs::remove_dir_all(&split_dir);
        logger::info("已清理临时拆分目录");
    }

    restore_and_exit(0);
}

// =============================================================================
// 公共流程函数
// =============================================================================

fn setup_signal_handler() -> Arc<AtomicBool> {
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
    running
}

fn init_environment(output_dir: &str) -> Result<(), String> {
    logger::step("初始化环境");

    let serial = adb::get_serialno()?;
    logger::info(&format!("ADB 设备已连接: {serial}"));

    adb::shell_cmd("svc power stayon true")?;
    logger::info("屏幕常亮已开启");

    fs::create_dir_all(output_dir).map_err(|e| format!("创建输出目录失败: {e}"))?;
    adb::shell_cmd(&format!("mkdir -p '{}'", config::PHONE_VIDEO_DIR))?;

    logger::info("环境初始化完成");
    Ok(())
}

fn collect_amproj_files(dir: &str) -> Vec<PathBuf> {
    let mut projects: Vec<_> = fs::read_dir(dir)
        .expect(&format!("无法读取目录: {dir}"))
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "amproj"))
        .map(|e| e.path())
        .collect();
    projects.sort();
    projects
}

fn batch_render(projects: &[PathBuf], output_dir: &str, running: &Arc<AtomicBool>) {
    let total = projects.len();
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

        match process_single_project(proj_path, output_dir) {
            Ok(()) => success += 1,
            Err(e) => {
                failed += 1;
                logger::error(&format!("工程处理失败: {e}，继续下一个..."));
            }
        }
    }

    println!();
    logger::step("==========================================");
    logger::step("全部处理完成");
    logger::step(&format!("  成功: {success}/{total}"));
    logger::step(&format!("  失败: {failed}/{total}"));
    logger::step(&format!("  输出目录: {output_dir}/"));
    logger::step("==========================================");
}

fn process_single_project(proj_path: &std::path::Path, output_dir: &str) -> Result<(), String> {
    let proj_filename = proj_path
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let proj_name = proj_path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let delay = || std::thread::sleep(std::time::Duration::from_millis(config::UI_STEP_DELAY_MS));
    let timeout = config::UI_WAIT_TIMEOUT_SECS;

    logger::step("==========================================");
    logger::step(&format!("开始处理工程: {proj_filename}"));
    logger::step("==========================================");

    // 阶段 0: 分析 amproj 类型
    let (proj_type, proj_title) = amproj::analyze(proj_path)?;
    logger::info(&format!(
        "工程类型: {proj_type}, 内部标题: \"{proj_title}\""
    ));

    // 阶段 1: 推送文件到手机
    logger::step("[1/9] 推送工程文件");
    adb::push_file(
        proj_path.to_str().unwrap(),
        &format!("{}/{}", config::PHONE_DOWNLOAD_DIR, proj_filename),
    )?;

    // 阶段 2: 查询 MediaStore ID 并通过 content:// URI 触发导入
    logger::step("[2/9] 通过 content:// URI 触发导入");
    std::thread::sleep(std::time::Duration::from_secs(2)); // MediaStore 索引需要时间
    let media_id = adb::query_media_id(&proj_filename)?;
    adb::open_project_via_content(&media_id)?;
    delay();

    // 阶段 3: 在导入确认对话框中点击"导入"（带重试）
    logger::step("[3/9] 确认导入对话框");
    if !ui::wait_and_tap_text("导入", timeout) {
        // 首次失败，可能 Alemon 冷启动慢，重新发送 intent
        logger::warn("导入对话框未出现，重试中...");
        adb::force_stop();
        std::thread::sleep(std::time::Duration::from_secs(2));
        adb::open_project_via_content(&media_id)?;
        std::thread::sleep(std::time::Duration::from_secs(1));
        if !ui::wait_and_tap_text("导入", 15) {
            adb::force_stop();
            adb::cleanup_phone(&proj_filename, None);
            return Err("无法找到导入确认按钮".into());
        }
    }
    delay();

    // 阶段 4: 在导入完成对话框中点击"完成"
    logger::step("[4/9] 等待导入完成");
    if !ui::wait_and_tap_text("完成", timeout) {
        logger::warn("未找到完成按钮，尝试关闭弹窗...");
        ui::dismiss_popups();
    }
    delay();

    // 阶段 5: 启动 Alemon 并根据类型导航到对应标签页
    logger::step("[5/9] 打开 Alemon 并导航到对应标签页");
    adb::launch_app()?;
    delay();

    let (tab_id, tab_text) = match proj_type {
        amproj::AmprojType::Project => ("tab_button_projects", "项目"),
        amproj::AmprojType::Element => ("tab_button_elements", "元素"),
    };
    logger::info(&format!("导航到「{tab_text}」标签"));

    if !ui::wait_and_tap_by_resource_id(tab_id, timeout) {
        if !ui::wait_and_tap_text(tab_text, timeout) {
            adb::force_stop();
            adb::cleanup_phone(&proj_filename, None);
            return Err(format!("无法导航到{tab_text}标签页"));
        }
    }
    delay();

    // 阶段 6: 在列表中找到刚导入的工程并打开
    logger::step("[6/9] 打开导入的工程");
    if !ui::wait_and_tap_text(&proj_title, timeout) {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err(format!("在列表中未找到工程: {proj_title}"));
    }
    delay();

    // 编辑器加载后可能连续出现多个弹窗（"原件丢失"、"缺失字体"等）
    // 同时等待 share 按钮出现，在等待过程中持续处理弹窗
    // 阶段 7: 点击导出菜单并开始渲染
    logger::step("[7/9] 打开导出菜单并开始渲染");

    {
        let share_timeout = 30u64; // 大项目编辑器加载+弹窗处理可能需要较长时间
        let deadline = std::time::Instant::now()
            + std::time::Duration::from_secs(share_timeout);
        let mut found_share = false;

        while std::time::Instant::now() < deadline {
            // 先尝试处理弹窗
            ui::dismiss_popups();

            // 然后检查 share 按钮
            if let Ok(xml) = adb::dump_ui_xml() {
                if let Some((cx, cy)) = ui::find_element_by_id(&xml, "share") {
                    logger::info(&format!("找到 share 按钮: ({cx}, {cy})，执行点击"));
                    let _ = adb::tap(cx, cy);
                    found_share = true;
                    break;
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(config::UI_POLL_INTERVAL_MS));
        }

        if !found_share {
            adb::force_stop();
            adb::cleanup_phone(&proj_filename, None);
            return Err("无法找到分享/导出按钮".into());
        }
    }
    delay();

    // 等待导出菜单加载（"视频"选项可见）
    if !ui::wait_for_text_visible("视频", timeout) {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err("导出菜单未正确加载".into());
    }

    // 点击 exportButton 开始渲染视频
    if !ui::wait_and_tap_by_resource_id("exportButton", timeout) {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err("无法找到导出按钮".into());
    }

    // 阶段 8: 等待渲染完成
    // exportButton 点击后：
    //   - 简单项目：瞬间渲染完成，直接进入预览页（含 saveButton）
    //   - 复杂项目：编辑器上覆盖渲染进度层，渲染完成后才进入预览页
    //   - 可能弹出"无法导出"警告，需点"仍要导出"继续
    logger::step("[8/9] 等待渲染完成");

    let render_timeout = config::MAX_RENDER_WAIT_SECS;
    let deadline = std::time::Instant::now()
        + std::time::Duration::from_secs(render_timeout);
    let mut last_log = std::time::Instant::now();
    let mut found_save = false;

    loop {
        if std::time::Instant::now() >= deadline {
            adb::force_stop();
            adb::cleanup_phone(&proj_filename, None);
            return Err(format!("渲染超时（{}s）", render_timeout));
        }

        // 处理可能的弹窗（"仍要导出"、"确定"等）
        ui::dismiss_popups();

        // 检测 saveButton（渲染完成标志）
        if let Ok(xml) = adb::dump_ui_xml() {
            if ui::find_element_by_id(&xml, "saveButton").is_some() {
                logger::info("渲染完成，预览页已出现");
                found_save = true;
                break;
            }

            // 渲染期间输出进度日志
            if last_log.elapsed() >= std::time::Duration::from_secs(15) {
                // 尝试检测渲染进度文本
                let elapsed = std::time::Instant::now()
                    .duration_since(deadline - std::time::Duration::from_secs(render_timeout));
                logger::info(&format!("  渲染中... 已等待 {}s", elapsed.as_secs()));
                last_log = std::time::Instant::now();
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(config::UI_POLL_INTERVAL_MS));
    }

    if !found_save {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err("渲染完成后未找到保存按钮".into());
    }

    // 点击 saveButton 保存文件
    if !ui::wait_and_tap_by_resource_id("saveButton", timeout) {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err("无法点击保存按钮".into());
    }

    // 等待文件写入完成
    logger::step("[8/9] 等待文件保存完成");
    let video_remote_path = render_monitor::wait_for_render_complete(&proj_title)?;

    // 阶段 9: 拉取视频与清理
    logger::step("[9/9] 拉取视频并清理");
    let video_local_name = format!("{proj_name}.mp4");
    adb::pull_file(
        &video_remote_path,
        &format!("{}/{}", output_dir, video_local_name),
    )?;

    adb::force_stop();
    adb::cleanup_phone(&proj_filename, Some(&video_remote_path));

    logger::step(&format!("工程 [{proj_filename}] 处理完成 ✓"));
    println!();
    Ok(())
}

fn restore_and_exit(code: i32) -> ! {
    let _ = adb::shell_cmd("svc power stayon false");
    logger::info("屏幕常亮已恢复为默认策略");
    std::process::exit(code);
}
