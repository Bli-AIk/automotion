# Avalonia Android 伴生项目可行性评估

## 背景

当前 `automotion` 是一个基于 Rust + ADB 的 PC 端命令行工具，**必须连接电脑**才能使用。
为了覆盖没有电脑、不懂 CLI 的用户群体，提议开发一个 **纯手机端 APK**，在设备上直接完成 amproj 的批量导出。

## 方案概述

| 项目           | automotion (现有)       | automotion-app (提议)          |
|---------------|------------------------|-------------------------------|
| 平台          | PC (Linux/macOS/Win)    | Android 纯手机端               |
| 语言          | Rust                    | C# (Avalonia) 或 Kotlin       |
| UI 自动化     | ADB `uiautomator dump`  | Android AccessibilityService  |
| 文件操作      | `adb push/pull/shell`   | Android Storage API           |
| 用户界面      | CLI                     | GUI (Avalonia/Compose)        |

## 核心技术：无障碍服务 (AccessibilityService)

### 原理

Android AccessibilityService 可以：
- **读取屏幕上所有 UI 节点**（等同于 `uiautomator dump`）
- **执行点击、滑动**（等同于 `adb shell input tap`）
- **监听 UI 变化事件**（比 uiautomator 轮询更高效）

这是 Auto.js、Tasker 等自动化工具的底层机制，且 **不需要 root、不需要 ADB、不需要电脑**。

### 与 ADB uiautomator 的对比

| 特性              | ADB uiautomator          | AccessibilityService     |
|------------------|--------------------------|--------------------------|
| 需要电脑          | ✅ 是                     | ❌ 否                     |
| 需要 root         | ❌ 否                     | ❌ 否                     |
| 需要特殊权限       | USB 调试                  | 无障碍权限（用户手动开启）  |
| UI 节点获取方式    | dump XML → regex 解析     | AccessibilityNodeInfo 树  |
| 响应速度          | 慢（dump+cat 约 1-3s）    | 快（事件驱动，毫秒级）      |
| 点击可靠性        | 有 `could not get idle state` 问题 | 直接调用 `performAction`，可靠 |
| 适用设备范围       | 需要支持 USB 调试          | 几乎所有 Android 设备      |

### 可复用的核心逻辑

以下逻辑可以从 Rust 项目直接移植：

1. **amproj 解析与拆分** — ZIP + XML 解析，语言无关
2. **UI 导航状态机** — 9 阶段流程（导入→导航→编辑器→导出→渲染→保存→拉取）
3. **弹窗处理** — POPUP_DISMISS_TEXTS 列表和匹配逻辑
4. **渲染状态检测** — exportButton→saveButton 轮询策略
5. **文件监控** — 文件大小稳定性检测

## 框架选型评估

### 方案 A：Avalonia (C#/.NET)

**优势：**
- 跨平台 UI 框架，可同时生成 Android APK 和桌面版
- C# 生态完善，ZIP/XML 处理便利
- MVVM 架构清晰
- 可在同一代码库中维护 PC 和手机版

**劣势：**
- Avalonia Android 支持相对较新（2023 年稳定）
- .NET Runtime 导致 APK 体积较大（~30-50MB）
- 调用 Android 原生 API（AccessibilityService）需要通过 Android 绑定
- 社区 Android 相关资源有限

**综合评分：⭐⭐⭐ (3/5)**

### 方案 B：Kotlin + Jetpack Compose

**优势：**
- Android 原生开发，AccessibilityService 集成最自然
- APK 体积小（~5-10MB）
- 性能最优，与 Alemon 同在 Android 环境中运行
- Google 官方支持，资源丰富
- Kotlin 的 ZIP/XML 处理能力完全满足需求

**劣势：**
- 仅限 Android 平台
- 与 Rust 代码库无法直接复用（但逻辑可移植）
- 需要学习 Kotlin（如果不熟悉）

**综合评分：⭐⭐⭐⭐⭐ (5/5)**

### 方案 C：Flutter (Dart)

**优势：**
- 跨平台（Android + iOS + 桌面）
- 丰富的 UI 组件和动画支持
- 热重载开发效率高

**劣势：**
- 调用 AccessibilityService 需要编写 Platform Channel（Java/Kotlin 桥接）
- Dart 的 ZIP 处理不如 Kotlin/C# 方便
- 运行时体积较大

**综合评分：⭐⭐⭐ (3/5)**

## 推荐方案

**推荐 方案 B：Kotlin + Jetpack Compose + Rust FFI (UniFFI)**

核心思路：**Rust 负责核心逻辑，Kotlin 只做 Android 胶水层（UI + AccessibilityService）**。
通过 UniFFI 自动生成 Kotlin 绑定，避免维护两套代码。

## FFI 方案评估

### Rust → Android FFI 的选项

| 方案 | 说明 | 评分 |
|------|------|------|
| **UniFFI** (Mozilla) | 从 Rust 接口定义自动生成 Kotlin 绑定 | ⭐⭐⭐⭐⭐ |
| `jni` crate + 手写 | 直接写 JNI 绑定，灵活但繁琐 | ⭐⭐⭐ |
| `diplomat` | 类似 UniFFI 的代码生成，但更年轻 | ⭐⭐⭐⭐ |

### UniFFI 详解

[UniFFI](https://mozilla.github.io/uniffi-rs/) 是 Mozilla 开发的 FFI 工具，用于 Firefox Android、Signal 等生产项目。

**工作流程：**
1. 在 Rust 中定义 `#[uniffi::export]` 函数/结构体
2. UniFFI 自动生成 Kotlin 绑定代码（类型安全、错误处理完整）
3. `cargo-ndk` 交叉编译为 Android `.so`（arm64-v8a、armeabi-v7a）
4. Kotlin 端直接调用，就像调用普通 Kotlin 函数一样

**示例（Rust 端）：**
```rust
#[uniffi::export]
pub fn parse_amproj(data: Vec<u8>) -> Result<AmprojInfo, AmprojError> { ... }

#[uniffi::export]
pub fn split_amproj(data: Vec<u8>) -> Result<Vec<SplitResult>, AmprojError> { ... }

#[derive(uniffi::Record)]
pub struct AmprojInfo {
    pub title: String,
    pub proj_type: String,  // "project" | "element"
    pub element_count: u32,
}
```

**自动生成的 Kotlin 代码：**
```kotlin
// 自动生成，无需手写
fun parseAmproj(data: ByteArray): AmprojInfo { ... }
fun splitAmproj(data: ByteArray): List<SplitResult> { ... }

data class AmprojInfo(
    val title: String,
    val projType: String,
    val elementCount: UInt,
)
```

### 代码复用划分

| 模块 | 位置 | 原因 |
|------|------|------|
| amproj 解析 (`amproj.rs`) | **Rust (FFI)** | 核心逻辑，完全复用 |
| amproj 拆分 (`split.rs`) | **Rust (FFI)** | 核心逻辑，完全复用 |
| 配置常量 (`config.rs`) | **Rust (FFI)** | 弹窗文本列表等，一处维护 |
| UI 元素查找 | **Kotlin** | AccessibilityService 原生 API |
| 弹窗处理逻辑 | **Rust (FFI)** 判断 + **Kotlin** 执行 | Rust 判断该点什么，Kotlin 执行点击 |
| 渲染监控 | **Kotlin** | 文件系统监听用 Android API 更高效 |
| UI 界面 | **Kotlin (Compose)** | 纯 Android UI |

**最终效果：约 60-70% 的核心逻辑在 Rust 中维护，只需写一次。**

### 构建工具链

```bash
# 1. 安装 Android NDK 交叉编译支持
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo install cargo-ndk

# 2. 构建 Android .so
cargo ndk -t arm64-v8a -t armeabi-v7a build --release

# 3. UniFFI 自动生成 Kotlin 绑定
# （通过 build.rs 或 gradle 插件自动完成）
```

## 项目架构草案

```
automotion/                          # 现有 Rust 项目 (monorepo)
├── Cargo.toml                       # workspace
├── crates/
│   ├── automotion-core/             # 核心库 (FFI 导出)
│   │   ├── Cargo.toml               # [lib] crate-type = ["cdylib", "lib"]
│   │   ├── src/
│   │   │   ├── lib.rs               # UniFFI 导出入口
│   │   │   ├── amproj.rs            # ← 从现有代码移入
│   │   │   ├── split.rs             # ← 从现有代码移入
│   │   │   └── config.rs            # ← 从现有代码移入
│   │   └── uniffi.toml
│   └── automotion-cli/              # CLI 二进制 (现有功能)
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs              # ← 现有 main.rs
│           ├── adb.rs               # ADB 交互（CLI 专用）
│           ├── ui.rs                # uiautomator 交互（CLI 专用）
│           └── render_monitor.rs    # 文件监控（CLI 专用）
├── app/                             # Android 项目
│   ├── app/
│   │   ├── src/main/java/.../
│   │   │   ├── MainActivity.kt
│   │   │   ├── AutomationService.kt # AccessibilityService
│   │   │   └── ui/                  # Compose UI
│   │   └── src/main/jniLibs/        # ← cargo-ndk 输出的 .so
│   └── build.gradle.kts
├── readme.md
└── readme_zh-hans.md
```

## 关键挑战

1. **无障碍权限引导** — 用户需要手动在系统设置中开启无障碍权限，需要做好引导流程
2. **后台保活** — 渲染时间可能很长（30分钟），需要前台服务 (Foreground Service) 防止被系统杀死
3. **文件访问** — Android 11+ 的 Scoped Storage 限制，可能需要 `MANAGE_EXTERNAL_STORAGE` 权限
4. **Alemon 版本兼容** — 不同版本的 UI 结构可能不同，需要适配
5. **UniFFI 学习曲线** — 需要熟悉 UniFFI 的类型映射和错误处理约定

## 结论

**方案完全可行。**

- AccessibilityService 比 ADB 更快、更可靠、不需要电脑
- UniFFI 让 Rust 核心代码直接被 Kotlin 调用，避免维护两套代码
- 建议使用 Kotlin + Jetpack Compose 做 Android 壳，Rust 做核心引擎
- 现有项目可重构为 workspace：`automotion-core`（共享库）+ `automotion-cli`（PC 工具）
- 未来 `automotion-core` 还可以支持 WASM、iOS 等平台
