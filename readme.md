# automotion

[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-APACHE) <img src="https://img.shields.io/github/repo-size/Bli-AIk/automotion.svg"/> <img src="https://img.shields.io/github/last-commit/Bli-AIk/automotion.svg"/> <br>
<img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" />

> Current Status: 🚧 Early Development

**automotion** — An experimental tool for automated batch exporting of Alight Motion project files.

| English         | Simplified Chinese            |
|-----------------|-------------------------------|
| English | [简体中文](./readme_zh-hans.md) |

## Introduction

`automotion` is a Rust-based automation tool for batch-processing `.amproj` (Alight Motion project) files into rendered videos.  
It solves the tedious manual workflow of importing, exporting, and rendering Alight Motion projects one by one.

The project provides two interfaces:
- **CLI Tool** (`automotion-cli`) — Desktop ADB automation that controls the phone from a Linux host
- **Android App** — On-device companion app with AccessibilityService-based UI automation

Both share the same Rust core library (`automotion-core`) via [UniFFI](https://mozilla.github.io/uniffi-rs/) for `.amproj` parsing and splitting logic.

## Features

* **Batch render** — Automatically process multiple `.amproj` files and export them as MP4 videos
* **Split** — Split a large `.amproj` project into individual element files
* **Split-render** — Split a project and then batch-render each element as a separate video
* **Group & Split** — Group all elements into a single embedScene, then split by frame intervals for time-based batch rendering
* **Fix** — Repair resource URIs (`amproj:` → `am:SHA1.ext`) to prevent texture/media loss across devices
* **Dynamic UI interaction** — Finds UI elements by `resource-id` and `text` (no hardcoded coordinates)
* **Render state detection** — Monitors output file stability to detect render completion
* **Popup handling** — Automatically dismisses common popups (missing fonts, missing media, ad prompts)
* **Clean workflow** — Cleans up phone-side files after each project to prevent storage bloat

## Architecture

```
automotion/
├── crates/
│   ├── automotion-core/    # Shared Rust library (amproj parsing, splitting, fixing, config)
│   │   └── src/
│   │       ├── amproj.rs   # .amproj analysis (ZIP/XML parsing)
│   │       ├── split.rs    # Project splitting logic
│   │       ├── group_split.rs # Group + time-based splitting
│   │       ├── fix.rs      # Resource path fix (amproj: → am:SHA1.ext)
│   │       ├── ffi.rs      # UniFFI FFI exports
│   │       └── ui_parser.rs # uiautomator XML parsing
│   └── automotion-cli/     # Desktop CLI tool (ADB commands)
└── app/                    # Android companion app (Kotlin + Compose)
    └── app/src/main/java/com/bliailk/automotion/
        ├── MainActivity.kt           # UI: file picker, mode selector, log viewer
        ├── RenderEngine.kt           # 9-stage automation state machine
        ├── AutomationService.kt      # AccessibilityService for UI automation
        └── RenderForegroundService.kt # Foreground service for background work
```

## Fix Tool

When sharing `.amproj` files between devices, embedded resources may fail to load due to a bug in Alemon's import URI remapping (particularly with non-ASCII filenames).

The `fix` command rewrites resource URIs from `amproj:filename` to `am:SHA1.ext`, which is the internal format Alemon uses after import. This bypasses the buggy remapping step while keeping all resources embedded in the ZIP.

```bash
# Fix a single file
automotion fix run my_project.amproj

# Fix all amproj files in input_projects/
automotion fix run

# Specify output directory
automotion fix run my_project.amproj -o ./fixed_output
```

The fixed file is saved as `{title}_fixed.amproj` in the output directory.

## Group & Split Tool

For projects with many overlapping elements, the `group-split` command groups all elements into a single embedScene container and then splits the timeline into fixed-frame chunks. This enables time-based batch rendering of complex compositions.

```bash
# Group all elements and split into 30-frame chunks (default)
automotion group-split my_project.amproj

# Custom frame count and output directory
automotion group-split my_project.amproj -f 60 -o ./my_output

# Group, split, and fix resource paths (prevents texture loss)
automotion group-split my_project.amproj --fix
```

The `group` command performs only the grouping step (no splitting):

```bash
automotion group my_project.amproj -o ./output_group
```

## Android App

The Android companion app runs directly on the device, using AccessibilityService to automate Alemon's UI. This avoids the limitations of ADB-based `uiautomator` (which is slower and less reliable).

### How It Works

1. Select `.amproj` files via the system file picker (supports multi-select)
2. Choose a mode: **Render**, **Split**, or **Split & Render**
3. The app launches Alemon with each file, navigates its UI to trigger export, monitors render progress, and moves to the next file automatically

### Key Technical Details

- **Background Activity Launch**: On Android 12+, the app uses natural BACK navigation (instead of `force-stop`) to return from Alemon to its own Activity, maintaining foreground status to bypass background activity launch restrictions
- **Cross-window node search**: The AccessibilityService searches all windows (not just the active one) for UI elements
- **9-stage pipeline**: Import → Wait for load → Navigate to export → Configure export → Confirm export → Wait for render → Save → Clean up → Next file

### Building the Android App

**Prerequisites:**
- Android SDK with NDK installed
- `cargo-ndk`: `cargo install cargo-ndk`
- Rust Android targets: `rustup target add aarch64-linux-android armv7-linux-androideabi`

**Build steps:**

```bash
# 1. Build Rust .so and generate UniFFI Kotlin bindings
cd app && bash build-rust.sh

# 2. Build the APK
./gradlew assembleDebug

# 3. Install on device
adb install -r app/build/outputs/apk/debug/app-debug.apk
```

### Setup on Device

1. **Enable AccessibilityService**: Settings → Accessibility → Automotion → Enable
2. **Grant storage permissions**: The app will request them on first launch
3. **Install Alemon**: Package name must be `com.taffy.alemon`
4. Open the app, select `.amproj` files, choose a mode, and tap Start

## CLI Tool

The CLI tool runs on a Linux host and controls the phone remotely via ADB.

### Usage

```bash
# Place .amproj files in input_projects/
cargo run -p automotion-cli -- render
cargo run -p automotion-cli -- split <file.amproj>
cargo run -p automotion-cli -- split-render <file.amproj>
cargo run -p automotion-cli -- group-split <file.amproj>
cargo run -p automotion-cli -- group-split <file.amproj> --fix
```

### Prerequisites

* Rust 1.85+ (edition 2024)
* ADB installed and in PATH
* Android device via USB with debugging enabled
* Alemon app installed (package: `com.taffy.alemon`)

## How to Build

```bash
git clone https://github.com/Bli-AIk/automotion.git
cd automotion
cargo build --release
```

## Dependencies

### Rust Crates

| Crate                                           | Version | Description                          |
| ----------------------------------------------- | ------- | ------------------------------------ |
| [regex](https://crates.io/crates/regex)         | 1       | Regular expression parsing for UI XML |
| [clap](https://crates.io/crates/clap)           | 4       | Command-line argument parsing         |
| [zip](https://crates.io/crates/zip)             | 2       | ZIP archive reading/writing for amproj |
| [roxmltree](https://crates.io/crates/roxmltree) | 0.20    | XML parsing for amproj internals     |
| [chrono](https://crates.io/crates/chrono)       | 0.4     | Timestamp formatting for logs         |
| [ctrlc](https://crates.io/crates/ctrlc)         | 3       | Signal handling for clean shutdown    |
| [uniffi](https://crates.io/crates/uniffi)       | 0.29    | Rust↔Kotlin FFI bridge               |

### Android App

* Kotlin + Jetpack Compose
* Android SDK 35, minSdk 26
* UniFFI-generated Kotlin bindings for `automotion-core`

## Contributing

Contributions are welcome!
Whether you want to fix a bug, add a feature, or improve documentation:

* Submit an **Issue** or **Pull Request**.
* Share ideas and discuss design or architecture.

## License

This project is licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0))
* MIT license ([LICENSE-MIT](LICENSE-MIT) or [http://opensource.org/licenses/MIT](http://opensource.org/licenses/MIT))

at your option.
