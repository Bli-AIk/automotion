// =============================================================================
// 日志模块 — 带时间戳的终端输出
// =============================================================================

use chrono::Local;

fn timestamp() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn info(msg: &str) {
    println!("[{}] [INFO]  {}", timestamp(), msg);
}

pub fn warn(msg: &str) {
    println!("[{}] [WARN]  {}", timestamp(), msg);
}

pub fn error(msg: &str) {
    eprintln!("[{}] [ERROR] {}", timestamp(), msg);
}

pub fn step(msg: &str) {
    println!("[{}] [STEP]  >>> {}", timestamp(), msg);
}
