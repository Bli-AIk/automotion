mod adb;
mod render_monitor;
mod ui;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Parser, Subcommand};

use automotion_core::{amproj, config, fix, group_split, logger, split};

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
        /// 渲染前先修复资源路径
        #[arg(long)]
        fix: bool,
        /// 同时导出项目包（amproj）
        #[arg(long)]
        amproj: bool,
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
        /// 渲染前先修复资源路径
        #[arg(long)]
        fix: bool,
        /// 同时导出项目包（amproj）
        #[arg(long)]
        amproj: bool,
    },
    /// 编组后按帧切分 amproj
    GroupSplit {
        /// 要处理的 amproj 文件路径
        file: String,
        /// 每段帧数
        #[arg(short, long, default_value = "30")]
        frames: u32,
        /// 输出目录
        #[arg(short, long, default_value = "./output_group_split")]
        output: String,
        /// 对输出文件执行 fix（修复资源路径，防止贴图丢失）
        #[arg(long, default_value = "false")]
        fix: bool,
    },
    /// 仅编组 amproj（不切分）
    Group {
        /// 要处理的 amproj 文件路径
        file: String,
        /// 输出目录
        #[arg(short, long, default_value = "./output_group")]
        output: String,
    },
    /// 修补 amproj 资源路径
    Fix {
        #[command(subcommand)]
        action: FixAction,
    },
}

#[derive(Subcommand)]
enum FixAction {
    /// 修复 amproj 资源路径（amproj: → am:SHA1.ext）
    Run {
        /// amproj 文件路径（默认处理 input_projects/ 下所有 amproj）
        file: Option<String>,
        /// 输出目录
        #[arg(short, long, default_value = "./fix_output")]
        output: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        None | Some(Commands::Render { .. }) => {
            // 默认模式 / render 子命令
            let (input, output, do_fix, do_amproj) = match cli.command {
                Some(Commands::Render {
                    input,
                    output,
                    fix,
                    amproj,
                }) => (input, output, fix, amproj),
                _ => (
                    "./input_projects".to_string(),
                    "./output_videos".to_string(),
                    false,
                    false,
                ),
            };
            run_render(&input, &output, do_fix, do_amproj);
        }
        Some(Commands::Split { file, output }) => {
            run_split(&file, &output);
        }
        Some(Commands::SplitRender {
            file,
            output,
            fix,
            amproj,
        }) => {
            run_split_render(&file, &output, fix, amproj);
        }
        Some(Commands::GroupSplit {
            file,
            frames,
            output,
            fix: do_fix,
        }) => {
            run_group_split(&file, &output, frames, do_fix);
        }
        Some(Commands::Group { file, output }) => {
            run_group(&file, &output);
        }
        Some(Commands::Fix { action }) => {
            run_fix(action);
        }
    }
}

// =============================================================================
// 子命令实现
// =============================================================================

/// render: 批量渲染 input 目录中的 amproj 文件
fn run_render(input_dir: &str, output_dir: &str, do_fix: bool, do_amproj: bool) {
    println!("============================================");
    println!("  automotion — Alemon 批量渲染自动化");
    println!("============================================");
    println!();

    let running = setup_signal_handler();

    // --fix: 先修复所有 amproj
    let (projects, fix_dir) = if do_fix {
        let fix_dir = PathBuf::from(".fix_tmp");
        let _ = fs::remove_dir_all(&fix_dir);
        let originals = collect_amproj_files(input_dir);
        let fixed = fix_batch(&originals, &fix_dir);
        (fixed, Some(fix_dir))
    } else {
        (collect_amproj_files(input_dir), None)
    };

    if let Err(e) = init_environment(output_dir) {
        logger::error(&format!("环境初始化失败: {e}"));
        std::process::exit(1);
    }

    if projects.is_empty() {
        logger::warn(&format!("在 {input_dir}/ 下未找到任何 .amproj 文件"));
        restore_and_exit(0);
    }

    if do_fix {
        batch_render_with_copy(&projects, output_dir, &running, do_amproj);
    } else {
        batch_render(&projects, output_dir, &running, do_amproj);
    }

    if let Some(dir) = fix_dir {
        let _ = fs::remove_dir_all(&dir);
    }
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
fn run_split_render(file: &str, output_dir: &str, do_fix: bool, do_amproj: bool) {
    println!("============================================");
    println!("  automotion — 拆分 + 批量渲染");
    println!("============================================");
    println!();

    let input_path = PathBuf::from(file);
    if !input_path.exists() {
        logger::error(&format!("文件不存在: {file}"));
        std::process::exit(1);
    }

    // --fix: 先修复原始文件
    let (actual_input, fix_dir) = if do_fix {
        let fix_dir = PathBuf::from(".fix_tmp");
        let _ = fs::remove_dir_all(&fix_dir);
        match fix::fix_amproj(&input_path, &fix_dir) {
            Ok(result) => {
                logger::step(&format!("修复完成: {}", result.output_amproj.display()));
                (result.output_amproj, Some(fix_dir))
            }
            Err(e) => {
                logger::error(&format!("修复失败: {e}"));
                std::process::exit(1);
            }
        }
    } else {
        (input_path, None)
    };

    // 阶段 1: 拆分到临时目录
    let split_dir = PathBuf::from("./.automotion_split_tmp");
    let mut split_outputs = match split::split_amproj(&actual_input, &split_dir) {
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

    // --fix: 修复拆分后的每个元素
    let fix_split_dir;
    if do_fix {
        fix_split_dir = PathBuf::from(".fix_split_tmp");
        let _ = fs::remove_dir_all(&fix_split_dir);
        split_outputs = fix_batch(&split_outputs, &fix_split_dir);
        logger::step(&format!("已修复 {} 个拆分元素", split_outputs.len()));
    } else {
        fix_split_dir = PathBuf::new();
    }

    // 阶段 2: 批量渲染
    let running = setup_signal_handler();

    if let Err(e) = init_environment(output_dir) {
        logger::error(&format!("环境初始化失败: {e}"));
        std::process::exit(1);
    }

    if do_fix {
        batch_render_with_copy(&split_outputs, output_dir, &running, do_amproj);
    } else {
        batch_render(&split_outputs, output_dir, &running, do_amproj);
    }

    // 清理临时目录
    for dir in [&split_dir, &fix_split_dir] {
        if dir.exists() {
            let _ = fs::remove_dir_all(dir);
        }
    }
    if let Some(dir) = fix_dir {
        if dir.exists() {
            let _ = fs::remove_dir_all(&dir);
        }
    }
    logger::info("已清理临时目录");

    restore_and_exit(0);
}

/// group-split: 编组后按帧切分
fn run_group_split(file: &str, output_dir: &str, frames: u32, do_fix: bool) {
    println!("============================================");
    println!("  automotion — amproj 编组+帧切分");
    println!("============================================");
    println!();

    let input_path = PathBuf::from(file);
    if !input_path.exists() {
        logger::error(&format!("文件不存在: {file}"));
        std::process::exit(1);
    }

    let output_path = PathBuf::from(output_dir);
    match group_split::group_and_split(&input_path, &output_path, frames) {
        Ok(outputs) => {
            logger::step(&format!("编组+切分完成，共 {} 个片段", outputs.len()));

            if do_fix {
                logger::step("开始对输出文件执行 fix（资源路径修复）...");
                let fix_output_dir = output_path.join("fixed");
                std::fs::create_dir_all(&fix_output_dir).ok();
                let mut fix_ok = 0;
                let mut fix_err = 0;
                // 使用临时目录避免同名覆盖，然后重命名为原始文件名
                let tmp_dir = output_path.join("_fix_tmp");
                for out_file in &outputs {
                    let original_name = out_file
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    // 每次 fix 到独立临时目录，避免同名覆盖
                    let _ = std::fs::remove_dir_all(&tmp_dir);
                    std::fs::create_dir_all(&tmp_dir).ok();
                    match fix::fix_amproj(out_file, &tmp_dir) {
                        Ok(result) => {
                            // 将 fix 输出重命名为原始文件名
                            let final_path = fix_output_dir.join(&original_name);
                            if let Err(e) = std::fs::rename(&result.output_amproj, &final_path) {
                                logger::error(&format!(
                                    "重命名失败: {} → {} — {}",
                                    result.output_amproj.display(),
                                    final_path.display(),
                                    e
                                ));
                                fix_err += 1;
                            } else {
                                fix_ok += 1;
                            }
                        }
                        Err(e) => {
                            logger::error(&format!("fix 失败: {} — {}", out_file.display(), e));
                            fix_err += 1;
                        }
                    }
                }
                let _ = std::fs::remove_dir_all(&tmp_dir);
                logger::step(&format!(
                    "fix 完成: {fix_ok} 成功, {fix_err} 失败, 输出目录: {}",
                    fix_output_dir.display()
                ));
            }
        }
        Err(e) => {
            logger::error(&format!("编组+切分失败: {e}"));
            std::process::exit(1);
        }
    }
}

/// group: 仅编组（不切分）
fn run_group(file: &str, output_dir: &str) {
    println!("============================================");
    println!("  automotion — amproj 编组工具");
    println!("============================================");
    println!();

    let input_path = PathBuf::from(file);
    if !input_path.exists() {
        logger::error(&format!("文件不存在: {file}"));
        std::process::exit(1);
    }

    let output_path = PathBuf::from(output_dir);
    match group_split::group_amproj(&input_path, &output_path) {
        Ok(output) => {
            logger::step(&format!("编组完成: {}", output.display()));
        }
        Err(e) => {
            logger::error(&format!("编组失败: {e}"));
            std::process::exit(1);
        }
    }
}

/// 批量修复 amproj 文件，返回修复后的文件列表
fn fix_batch(files: &[PathBuf], output_dir: &PathBuf) -> Vec<PathBuf> {
    let mut fixed = Vec::new();
    for proj in files {
        match fix::fix_amproj(proj, output_dir) {
            Ok(result) => {
                fixed.push(result.output_amproj);
            }
            Err(e) => {
                logger::error(&format!("修复失败: {} — {e}", proj.display()));
                std::process::exit(1);
            }
        }
    }
    fixed
}

/// fix: 修补 amproj 资源路径
fn run_fix(action: FixAction) {
    println!("============================================");
    println!("  automotion — amproj 资源路径修补");
    println!("============================================");
    println!();

    match action {
        FixAction::Run { file, output } => {
            let output_path = PathBuf::from(&output);

            // 收集要处理的文件
            let files: Vec<PathBuf> = if let Some(f) = file {
                let p = PathBuf::from(&f);
                if !p.exists() {
                    logger::error(&format!("文件不存在: {f}"));
                    std::process::exit(1);
                }
                vec![p]
            } else {
                // 处理 input_projects/ 下所有 amproj
                let input_dir = PathBuf::from("./input_projects");
                if !input_dir.exists() {
                    logger::error("input_projects/ 目录不存在");
                    std::process::exit(1);
                }
                let mut found = Vec::new();
                if let Ok(entries) = fs::read_dir(&input_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().map_or(false, |e| e == "amproj") {
                            found.push(path);
                        }
                    }
                }
                if found.is_empty() {
                    logger::error("input_projects/ 中没有 amproj 文件");
                    std::process::exit(1);
                }
                found
            };

            for input_path in &files {
                match fix::fix_amproj(input_path, &output_path) {
                    Ok(result) => {
                        logger::step(&format!("修复完成: {}", result.output_amproj.display()));
                    }
                    Err(e) => {
                        logger::error(&format!("修补失败: {e}"));
                        std::process::exit(1);
                    }
                }
            }

            logger::step(&format!("共修复 {} 个文件", files.len()));
        }
    }
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

fn batch_render(
    projects: &[PathBuf],
    output_dir: &str,
    running: &Arc<AtomicBool>,
    do_amproj: bool,
) {
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

        match process_single_project(proj_path, output_dir, do_amproj) {
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

/// 渲染后同时将 amproj 复制到输出目录，文件名统一使用内部标题
fn batch_render_with_copy(
    projects: &[PathBuf],
    output_dir: &str,
    running: &Arc<AtomicBool>,
    do_amproj: bool,
) {
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

        // 获取内部标题作为统一文件名
        let title = match amproj::analyze(proj_path) {
            Ok((_type, title)) => title,
            Err(e) => {
                failed += 1;
                logger::error(&format!("分析失败: {e}，继续下一个..."));
                continue;
            }
        };

        match process_single_project(proj_path, output_dir, do_amproj) {
            Ok(()) => {
                // 渲染成功后：重命名 mp4（和 amproj），统一使用内部标题
                let proj_name = proj_path.file_stem().unwrap().to_string_lossy();
                let old_mp4 = format!("{}/{}.mp4", output_dir, proj_name);
                let new_mp4 = format!("{}/{}.mp4", output_dir, title);

                if old_mp4 != new_mp4 && PathBuf::from(&old_mp4).exists() {
                    let _ = fs::rename(&old_mp4, &new_mp4);
                }
                if do_amproj {
                    let old_amproj = format!("{}/{}.amproj", output_dir, proj_name);
                    let new_amproj = format!("{}/{}.amproj", output_dir, title);
                    if old_amproj != new_amproj && PathBuf::from(&old_amproj).exists() {
                        let _ = fs::rename(&old_amproj, &new_amproj);
                    }
                    logger::info(&format!("输出: {title}.mp4 + {title}.amproj"));
                } else {
                    logger::info(&format!("输出: {title}.mp4"));
                }
                success += 1;
            }
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

fn process_single_project(
    proj_path: &std::path::Path,
    output_dir: &str,
    do_amproj: bool,
) -> Result<(), String> {
    let proj_filename = proj_path.file_name().unwrap().to_string_lossy().to_string();
    let proj_name = proj_path.file_stem().unwrap().to_string_lossy().to_string();

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
    logger::step("[1/10] 推送工程文件");
    adb::push_file(
        proj_path.to_str().unwrap(),
        &format!("{}/{}", config::PHONE_DOWNLOAD_DIR, proj_filename),
    )?;

    // 阶段 2: 查询 MediaStore ID 并通过 content:// URI 触发导入
    logger::step("[2/10] 通过 content:// URI 触发导入");
    std::thread::sleep(std::time::Duration::from_secs(2)); // MediaStore 索引需要时间
    let media_id = adb::query_media_id(&proj_filename)?;
    adb::open_project_via_content(&media_id)?;
    delay();

    // 阶段 3: 在导入确认对话框中点击"导入"（带重试）
    logger::step("[3/10] 确认导入对话框");
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
    logger::step("[4/10] 等待导入完成");
    if !ui::wait_and_tap_text("完成", timeout) {
        logger::warn("未找到完成按钮，尝试关闭弹窗...");
        ui::dismiss_popups();
    }
    delay();

    // 阶段 5: 启动 Alemon 并根据类型导航到对应标签页
    logger::step("[5/10] 打开 Alemon 并导航到对应标签页");
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
    logger::step("[6/10] 打开导入的工程");
    if !ui::wait_and_tap_text(&proj_title, timeout) {
        adb::force_stop();
        adb::cleanup_phone(&proj_filename, None);
        return Err(format!("在列表中未找到工程: {proj_title}"));
    }
    delay();

    // 编辑器加载后可能连续出现多个弹窗（"原件丢失"、"缺失字体"等）
    // 同时等待 share 按钮出现，在等待过程中持续处理弹窗
    // 阶段 7: 点击导出菜单并开始渲染
    logger::step("[7/10] 打开导出菜单并开始渲染");

    {
        let share_timeout = 30u64; // 大项目编辑器加载+弹窗处理可能需要较长时间
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(share_timeout);
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
            std::thread::sleep(std::time::Duration::from_millis(
                config::UI_POLL_INTERVAL_MS,
            ));
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
    logger::step("[8/10] 等待渲染完成");

    let render_timeout = config::MAX_RENDER_WAIT_SECS;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(render_timeout);
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

        std::thread::sleep(std::time::Duration::from_millis(
            config::UI_POLL_INTERVAL_MS,
        ));
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
    logger::step("[8/10] 等待文件保存完成");
    let video_remote_path = render_monitor::wait_for_render_complete(&proj_title)?;

    // 阶段 9: 导出项目包（amproj）— 仅在 --amproj 模式下执行
    let amproj_remote_path = if do_amproj {
        logger::step("[9/10] 导出项目包");
        Some(export_project_package(
            &proj_title,
            &proj_filename,
            timeout,
        )?)
    } else {
        logger::info("[9/10] 跳过项目包导出（未指定 --amproj）");
        None
    };

    // 阶段 10: 拉取视频（和项目包），然后清理
    logger::step("[10/10] 拉取文件并清理");
    let video_local_name = format!("{proj_name}.mp4");
    adb::pull_file(
        &video_remote_path,
        &format!("{}/{}", output_dir, video_local_name),
    )?;

    if let Some(ref amproj_path) = amproj_remote_path {
        let amproj_local_name = format!("{proj_name}.amproj");
        adb::pull_file(
            amproj_path,
            &format!("{}/{}", output_dir, amproj_local_name),
        )?;
    }

    adb::force_stop();
    adb::cleanup_phone(&proj_filename, Some(&video_remote_path));
    if let Some(ref amproj_path) = amproj_remote_path {
        let _ = adb::shell_cmd(&format!("rm -f '{amproj_path}'"));
    }

    logger::step(&format!("工程 [{proj_filename}] 处理完成 ✓"));
    println!();
    Ok(())
}

fn restore_and_exit(code: i32) -> ! {
    let _ = adb::shell_cmd("svc power stayon false");
    logger::info("屏幕常亮已恢复为默认策略");
    std::process::exit(code);
}

/// 导出项目包：从编辑器的导出菜单导出 amproj 项目包
/// 返回手机端保存的 amproj 文件路径
fn export_project_package(
    proj_title: &str,
    _proj_filename: &str,
    timeout: u64,
) -> Result<String, String> {
    let delay = || std::thread::sleep(std::time::Duration::from_millis(config::UI_STEP_DELAY_MS));

    // 从视频预览页返回编辑器：点击"关闭"(doneButton)
    logger::info("从视频预览页返回编辑器...");
    if !ui::wait_and_tap_by_resource_id("doneButton", 5) {
        // 备选：按 back 键
        adb::press_back()?;
        delay();
    }
    delay();

    // 等待编辑器页面加载（share 按钮出现）
    let share_timeout = 15u64;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(share_timeout);
    let mut found_share = false;

    while std::time::Instant::now() < deadline {
        ui::dismiss_popups();
        if let Ok(xml) = adb::dump_ui_xml() {
            if let Some((cx, cy)) = ui::find_element_by_id(&xml, "share") {
                logger::info(&format!("找到 share 按钮: ({cx}, {cy})，打开导出菜单"));
                let _ = adb::tap(cx, cy);
                found_share = true;
                break;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(
            config::UI_POLL_INTERVAL_MS,
        ));
    }

    if !found_share {
        return Err("返回编辑器后未找到 share 按钮".into());
    }
    delay();

    // 等待导出菜单加载
    if !ui::wait_for_text_visible("项目包", timeout) {
        return Err("导出菜单中未找到「项目包」选项".into());
    }

    // 点击"项目包"标签
    if !ui::wait_and_tap_text("项目包", timeout) {
        return Err("无法点击「项目包」标签".into());
    }
    delay();

    // 点击 exportButton 导出项目包
    if !ui::wait_and_tap_by_resource_id("exportButton", timeout) {
        return Err("无法点击导出按钮".into());
    }

    // 等待打包完成（打包对话框 "打包项目" 出现并消失）
    logger::info("等待项目打包完成...");
    {
        let pack_deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout);
        // 先等打包对话框出现或分享面板直接出现
        loop {
            if let Ok(xml) = adb::dump_ui_xml() {
                // 打包完成后分享面板直接出现
                if xml.contains("com.huawei.android.internal.app") || xml.contains("另存为") {
                    logger::info("分享面板已出现");
                    break;
                }
            }
            if std::time::Instant::now() >= pack_deadline {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(
                config::UI_POLL_INTERVAL_MS,
            ));
        }
    }
    delay();

    // Android 分享面板出现 → 点击"另存为"
    if !ui::tap_save_as_in_share_sheet(timeout) {
        // 分享面板可能未出现或按钮不同，尝试关闭并重试
        adb::press_back()?;
        return Err("分享面板中未找到「另存为」按钮".into());
    }
    delay();

    // SAF 文件选择器 → 导航到 Download 并保存
    if !ui::save_in_saf_to_download(15) {
        adb::press_back()?;
        return Err("SAF 文件选择器中未能保存到 Download".into());
    }

    // 等待文件出现
    std::thread::sleep(std::time::Duration::from_secs(2));

    // 查找保存的 amproj 文件
    let amproj_path = adb::find_latest_amproj_in_download(proj_title)?;
    logger::info(&format!("项目包已保存: {amproj_path}"));

    // 关闭可能残留的文件管理器界面
    let _ = adb::press_back();
    delay();

    Ok(amproj_path)
}
