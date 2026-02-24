# automotion

[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-APACHE) <img src="https://img.shields.io/github/repo-size/Bli-AIk/automotion.svg"/> <img src="https://img.shields.io/github/last-commit/Bli-AIk/automotion.svg"/> <br>
<img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" />

> 当前状态：🚧 早期开发中

**automotion** — 一个自动化批量导出 Alight Motion 工程文件的实验性工具。

| English                | 简体中文   |
|------------------------|-----------|
| [English](./readme.md) | 简体中文  |

## 简介

`automotion` 是一个基于 Rust 的自动化工具，用于批量处理 `.amproj`（Alight Motion 工程）文件并渲染为视频。  
它解决了逐个手动导入、导出和渲染 Alight Motion 工程的繁琐工作流程。

本项目提供两种接口：
- **CLI 工具** (`automotion-cli`) — 桌面端 ADB 自动化，从 Linux 主机遥控手机
- **Android 应用** — 设备端伴侣应用，基于 AccessibilityService 的 UI 自动化

两者共享同一个 Rust 核心库 (`automotion-core`)，通过 [UniFFI](https://mozilla.github.io/uniffi-rs/) 实现 `.amproj` 解析和拆分逻辑的复用。

## 功能

* **批量渲染** — 自动处理多个 `.amproj` 文件并导出为 MP4 视频
* **拆分** — 将大型 `.amproj` 项目拆分为独立的元素文件
* **拆分渲染** — 拆分项目后将每个元素分别渲染为视频
* **编组切分** — 将所有元素编组为单一 embedScene，再按帧间隔切分，实现基于时间的批量渲染
* **修复** — 修复资源 URI（`amproj:` → `am:SHA1.ext`），防止跨设备传输时贴图/媒体丢失
* **动态 UI 交互** — 按 `resource-id` 和 `text` 查找 UI 元素（不硬编码坐标）
* **渲染状态检测** — 监控输出文件稳定性以检测渲染完成
* **弹窗处理** — 自动关闭常见弹窗（缺失字体、缺失媒体、广告提示等）
* **清洁工作流** — 每个项目处理完成后清理手机端文件，防止存储膨胀

## 架构

```
automotion/
├── crates/
│   ├── automotion-core/    # 共享 Rust 核心库（amproj 解析、拆分、修复、配置）
│   │   └── src/
│   │       ├── amproj.rs   # .amproj 分析（ZIP/XML 解析）
│   │       ├── split.rs    # 工程拆分逻辑
│   │       ├── group_split.rs # 编组 + 时间切分
│   │       ├── fix.rs      # 资源路径修复（amproj: → am:SHA1.ext）
│   │       ├── ffi.rs      # UniFFI FFI 导出
│   │       └── ui_parser.rs # uiautomator XML 解析
│   └── automotion-cli/     # 桌面 CLI 工具（ADB 命令）
└── app/                    # Android 伴侣应用（Kotlin + Compose）
    └── app/src/main/java/com/bliailk/automotion/
        ├── MainActivity.kt           # UI：文件选择、模式选择、日志查看
        ├── RenderEngine.kt           # 9 阶段自动化状态机
        ├── AutomationService.kt      # AccessibilityService UI 自动化
        └── RenderForegroundService.kt # 前台服务（后台工作）
```

## 修复工具

跨设备传输 `.amproj` 文件时，嵌入资源可能因 Alemon 导入时 URI 重映射 bug 而丢失（尤其是非 ASCII 文件名）。

`fix` 命令将资源 URI 从 `amproj:filename` 预转换为 `am:SHA1.ext`（Alemon 导入后的内部格式），绕过有 bug 的重映射步骤，同时保留 ZIP 内的嵌入资源。

```bash
# 修复单个文件
automotion fix run my_project.amproj

# 修复 input_projects/ 下所有 amproj 文件
automotion fix run

# 指定输出目录
automotion fix run my_project.amproj -o ./fixed_output
```

修复后的文件保存为 `{标题}_fixed.amproj`。

## 编组切分工具

对于包含大量重叠元素的工程，`group-split` 命令会将所有元素编组为单一 embedScene 容器，然后按固定帧数将时间线切分为多个片段。这使得复杂合成可以基于时间进行批量渲染。

```bash
# 编组所有元素并按 30 帧切分（默认）
automotion group-split my_project.amproj

# 自定义帧数和输出目录
automotion group-split my_project.amproj -f 60 -o ./my_output

# 编组、切分并修复资源路径（防止贴图丢失）
automotion group-split my_project.amproj --fix
```

`group` 命令仅执行编组步骤（不切分）：

```bash
automotion group my_project.amproj -o ./output_group
```

## Android 应用

Android 伴侣应用直接在设备上运行，使用 AccessibilityService 自动化 Alemon 的 UI。相比基于 ADB 的 `uiautomator` 更快、更可靠。

### 工作原理

1. 通过系统文件选择器选取 `.amproj` 文件（支持多选）
2. 选择模式：**渲染**、**拆分** 或 **拆分并渲染**
3. 应用依次启动 Alemon 处理每个文件，操作 UI 触发导出，监控渲染进度，自动跳转至下一个文件

### 关键技术细节

- **后台 Activity 启动**：在 Android 12+ 上，应用通过自然 BACK 导航（而非 `force-stop`）从 Alemon 返回自身 Activity，保持前台状态以绕过后台 Activity 启动限制
- **跨窗口节点搜索**：AccessibilityService 搜索所有窗口（不仅是活跃窗口）中的 UI 元素
- **9 阶段流水线**：导入 → 等待加载 → 导航到导出 → 配置导出 → 确认导出 → 等待渲染 → 保存 → 清理 → 下一个文件

### 构建 Android 应用

**前置要求：**
- 安装 Android SDK 及 NDK
- `cargo-ndk`：`cargo install cargo-ndk`
- Rust Android 目标：`rustup target add aarch64-linux-android armv7-linux-androideabi`

**构建步骤：**

```bash
# 1. 构建 Rust .so 并生成 UniFFI Kotlin 绑定
cd app && bash build-rust.sh

# 2. 构建 APK
./gradlew assembleDebug

# 3. 安装到设备
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

### 设备端设置

1. **启用无障碍服务**：设置 → 无障碍 → Automotion → 开启
2. **授予存储权限**：首次启动时应用会自动请求
3. **安装 Alemon**：包名必须为 `com.taffy.alemon`
4. 打开应用，选择 `.amproj` 文件，选择模式，点击开始

## CLI 工具

CLI 工具在 Linux 主机上运行，通过 ADB 远程控制手机。

### 使用方法

```bash
# 将 .amproj 文件放入 input_projects/
cargo run -p automotion-cli -- render
cargo run -p automotion-cli -- split <file.amproj>
cargo run -p automotion-cli -- split-render <file.amproj>
cargo run -p automotion-cli -- group-split <file.amproj>
cargo run -p automotion-cli -- group-split <file.amproj> --fix
```

### 前置要求

* Rust 1.85+（edition 2024）
* 已安装 ADB 并在 PATH 中
* Android 设备通过 USB 连接并开启调试模式
* 已安装 Alemon 应用（包名：`com.taffy.alemon`）

## 构建方法

```bash
git clone https://github.com/Bli-AIk/automotion.git
cd automotion
cargo build --release
```

## 依赖

### Rust Crates

| Crate                                           | 版本  | 说明                             |
| ----------------------------------------------- | ----- | -------------------------------- |
| [regex](https://crates.io/crates/regex)         | 1     | 正则表达式解析 UI XML             |
| [clap](https://crates.io/crates/clap)           | 4     | 命令行参数解析                    |
| [zip](https://crates.io/crates/zip)             | 2     | ZIP 归档读写（amproj 格式）       |
| [roxmltree](https://crates.io/crates/roxmltree) | 0.20  | XML 解析（amproj 内部结构）       |
| [chrono](https://crates.io/crates/chrono)       | 0.4   | 日志时间戳格式化                  |
| [ctrlc](https://crates.io/crates/ctrlc)         | 3     | 信号处理（优雅退出）              |
| [uniffi](https://crates.io/crates/uniffi)       | 0.29  | Rust↔Kotlin FFI 桥接             |

### Android 应用

* Kotlin + Jetpack Compose
* Android SDK 35，minSdk 26
* 通过 UniFFI 生成的 Kotlin 绑定调用 `automotion-core`

## 贡献

欢迎贡献！
无论是修复 bug、添加功能还是改进文档：

* 提交 **Issue** 或 **Pull Request**。
* 分享想法并讨论设计或架构。

## 许可证

本项目采用以下任一许可证授权：

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) 或 [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](LICENSE-MIT) 或 [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

任选其一。
