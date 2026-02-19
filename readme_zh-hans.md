# automotion

[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-APACHE) <img src="https://img.shields.io/github/repo-size/Bli-AIk/automotion.svg"/> <img src="https://img.shields.io/github/last-commit/Bli-AIk/automotion.svg"/> <br>
<img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" />

> 当前状态：🚧 早期开发中（初始版本开发中）

**automotion** — 一个通过 ADB 自动化批量导出 Alight Motion 工程文件的实验性工具。

| English                | 简体中文   |
|------------------------|-----------|
| [English](./readme.md) | 简体中文  |

## 简介

`automotion` 是一个基于 Rust 的 ADB 自动化工具，用于通过 USB 调试在 Android 设备上批量处理 `.amproj`（Alight Motion 工程）文件。  
它解决了逐个手动导入、导出和渲染 Alight Motion 工程的繁琐工作流程，允许用户自动化从推送到拉取的整个流水线。

使用 `automotion`，你只需将 `.amproj` 文件放入 `input_projects/` 目录并运行一条命令。  
工具会自动处理推送文件到手机、导入 Alemon、操作 UI、触发导出、监控渲染进度，以及拉取生成的 `.mp4` 视频。

## 功能

* **批量渲染** — 自动处理 `input_projects/` 中的所有 `.amproj` 文件并导出为 MP4 视频
* **拆分** — 将大型 `.amproj` 项目拆分为独立的元素文件
* **拆分渲染** — 拆分项目后将每个元素分别渲染为视频
* **动态 UI 交互** — 通过 `uiautomator` XML 解析按 `resource-id` 和 `text` 查找 UI 元素（不硬编码坐标）
* **渲染状态检测** — 轮询 `saveButton` 出现以检测渲染完成，支持长达 30 分钟的渲染
* **弹窗处理** — 自动关闭常见弹窗（缺失字体、缺失媒体、广告提示等）
* **设备保护** — 运行期间保持屏幕常亮，退出时恢复（包括 Ctrl+C 中断）
* **清洁工作流** — 每个项目处理完成后清理手机端文件，防止存储膨胀

## 使用方法

1. **安装 Rust**（如未安装）：
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **克隆仓库**：

   ```bash
   git clone https://github.com/Bli-AIk/automotion.git
   cd automotion
   ```

3. **构建并运行**：

   ```bash
   cargo run
   ```

4. **基本命令**：

   * 批量渲染所有工程：`cargo run -- render`
   * 将工程拆分为元素：`cargo run -- split <file.amproj>`
   * 拆分并渲染：`cargo run -- split-render <file.amproj>`

5. **环境配置**：
   * 通过 USB 连接 Android 设备并开启 ADB 调试
   * 在设备上安装 Alemon 应用（包名：`com.taffy.alemon`）
   * 将 `.amproj` 文件放入 `./input_projects/` 目录
   * 渲染完成的视频将保存到 `./output_videos/`

## 构建方法

### 前置要求

* Rust 1.85 或更高版本（edition 2024）
* 已安装 ADB（Android Debug Bridge）并在 PATH 中
* Android 设备通过 USB 连接并开启调试模式

### 构建步骤

1. **克隆仓库**：

   ```bash
   git clone https://github.com/Bli-AIk/automotion.git
   cd automotion
   ```

2. **构建项目**：

   ```bash
   cargo build --release
   ```

3. **运行测试**：

   ```bash
   cargo test
   ```

4. **全局安装**（可选）：

   ```bash
   cargo install --path .
   ```

## 依赖

本项目使用以下 crate：

| Crate                                           | 版本  | 说明                             |
| ----------------------------------------------- | ----- | -------------------------------- |
| [regex](https://crates.io/crates/regex)         | 1     | 正则表达式解析 UI XML             |
| [clap](https://crates.io/crates/clap)           | 4     | 命令行参数解析                    |
| [zip](https://crates.io/crates/zip)             | 2     | ZIP 归档读写（amproj 格式）       |
| [roxmltree](https://crates.io/crates/roxmltree) | 0.20  | XML 解析（amproj 内部结构）       |
| [chrono](https://crates.io/crates/chrono)       | 0.4   | 日志时间戳格式化                  |
| [ctrlc](https://crates.io/crates/ctrlc)         | 3     | 信号处理（优雅退出）              |

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
