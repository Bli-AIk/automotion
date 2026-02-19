// =============================================================================
// ADB 交互模块 — 封装所有 adb 命令调用
// =============================================================================

use std::process::Command;

use crate::config;
use crate::logger;

/// 执行 adb 命令并返回 stdout（去除尾部空白）
pub fn run(args: &[&str]) -> Result<String, String> {
    let output = Command::new("adb")
        .args(args)
        .output()
        .map_err(|e| format!("adb 命令执行失败: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!("adb {:?} 失败: {stderr}", args));
    }

    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string())
}

/// 执行 adb shell 命令
pub fn shell_cmd(cmd: &str) -> Result<String, String> {
    run(&["shell", cmd])
}

/// 获取设备序列号
pub fn get_serialno() -> Result<String, String> {
    run(&["get-serialno"])
}

/// 推送文件到手机
pub fn push_file(local: &str, remote: &str) -> Result<String, String> {
    logger::info(&format!("推送文件: {local} -> {remote}"));
    run(&["push", local, remote])
}

/// 从手机拉取文件
pub fn pull_file(remote: &str, local: &str) -> Result<String, String> {
    logger::info(&format!("拉取文件: {remote} -> {local}"));
    run(&["pull", remote, local])
}

/// 查询手机端文件的 MediaStore content:// ID
/// 返回数字 ID，用于构造 content://media/external/file/{id}
pub fn query_media_id(filename: &str) -> Result<String, String> {
    let output = shell_cmd(&format!(
        "content query --uri content://media/external/file --projection _id --where \"_display_name='{filename}'\"",
    ))?;
    // 输出格式: "Row: 0 _id=230142"
    let id = output
        .split("_id=")
        .nth(1)
        .and_then(|s| s.split_whitespace().next())
        .ok_or_else(|| format!("无法从 MediaStore 获取文件 ID: {output}"))?
        .to_string();
    logger::info(&format!("MediaStore ID for {filename}: {id}"));
    Ok(id)
}

/// 通过 content:// URI 唤醒 Alemon 导入工程
/// 必须使用 content:// 而非 file://（Android 7.0+ file:// 受限会导致 RuntimeException）
pub fn open_project_via_content(media_id: &str) -> Result<String, String> {
    logger::info(&format!(
        "通过 content://media/external/file/{media_id} 唤醒 Alemon"
    ));
    shell_cmd(&format!(
        "am start -a android.intent.action.VIEW \
         -d 'content://media/external/file/{media_id}' \
         -t application/zip \
         -p {} \
         --grant-read-uri-permission",
        config::PACKAGE_NAME,
    ))
}

/// 强制停止 Alemon
pub fn force_stop() {
    logger::info("强制停止 Alemon");
    let _ = shell_cmd(&format!("am force-stop {}", config::PACKAGE_NAME));
}

/// 启动 Alemon 主界面
pub fn launch_app() -> Result<String, String> {
    logger::info("启动 Alemon 主界面");
    shell_cmd(&format!(
        "monkey -p {} -c android.intent.category.LAUNCHER 1",
        config::PACKAGE_NAME
    ))
}

/// 清理手机端文件
pub fn cleanup_phone(proj_filename: &str, video_path: Option<&str>) {
    logger::info("清理手机端文件...");
    let _ = shell_cmd(&format!(
        "rm -f '{}/{}'",
        config::PHONE_DOWNLOAD_DIR,
        proj_filename
    ));
    if let Some(vp) = video_path {
        let _ = shell_cmd(&format!("rm -f '{vp}'"));
    }
    logger::info("手机端清理完成");
}

/// dump 当前屏幕 UI 树 XML
pub fn dump_ui_xml() -> Result<String, String> {
    shell_cmd(&format!(
        "uiautomator dump {} && cat {}",
        config::PHONE_UI_DUMP,
        config::PHONE_UI_DUMP
    ))
}

/// 执行屏幕点击
pub fn tap(x: i32, y: i32) -> Result<String, String> {
    shell_cmd(&format!("input tap {x} {y}"))
}
