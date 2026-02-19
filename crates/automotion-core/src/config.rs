// =============================================================================
// 全局配置常量（按需修改）
// =============================================================================

/// Alemon 包名（严格指定）
pub const PACKAGE_NAME: &str = "com.taffy.alemon"; // 实测实际包名

/// 手机端路径
pub const PHONE_DOWNLOAD_DIR: &str = "/sdcard/Download";
pub const PHONE_VIDEO_DIR: &str = "/sdcard/Movies/Alemon"; // 实测确认路径
pub const PHONE_UI_DUMP: &str = "/sdcard/window_dump.xml";

/// 本地路径
pub const LOCAL_INPUT_DIR: &str = "./input_projects";
pub const LOCAL_OUTPUT_DIR: &str = "./output_videos";

// === UI 文本特征词（需要根据实际 uiautomator dump 结果手动填入） ===
pub const UI_TEXT_EXPORT: &str = "导出";     // ← 导出按钮文本
pub const UI_TEXT_CONFIRM: &str = "确认";    // ← 确认导出按钮文本
pub const UI_TEXT_SAVE: &str = "保存";       // ← 保存按钮文本
pub const UI_TEXT_DISMISS_AD: &str = "跳过"; // ← 弹窗/广告跳过按钮文本

/// 弹窗关键词列表（遇到这些文字的按钮会自动点击关闭）
pub const POPUP_DISMISS_TEXTS: &[&str] = &["仍要导出", "跳过", "稍后", "不了", "跳过广告", "确定", "确认", "好"];

// === 渲染监控参数 ===
pub const POLL_INTERVAL_SECS: u64 = 3;        // 轮询间隔（秒）
pub const STABLE_THRESHOLD_SECS: u64 = 15;    // 文件大小无变化判定阈值（秒）
pub const MAX_RENDER_WAIT_SECS: u64 = 1800;   // 单个工程最大渲染等待时间（秒）

// === UI 交互时序参数（可根据设备性能调整） ===
pub const UI_POLL_INTERVAL_MS: u64 = 500;     // UI 元素轮询间隔（毫秒）
pub const UI_WAIT_TIMEOUT_SECS: u64 = 10;     // UI 元素等待超时（秒），导出阶段不受此限制
pub const UI_STEP_DELAY_MS: u64 = 500;        // 步骤间延时（毫秒）
pub const UI_RETRY_COUNT: u32 = 10;           // 备用：旧式 UI 元素查找重试次数
