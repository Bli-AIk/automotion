# automotion

[![license](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue)](LICENSE-APACHE) <img src="https://img.shields.io/github/repo-size/Bli-AIk/automotion.svg"/> <img src="https://img.shields.io/github/last-commit/Bli-AIk/automotion.svg"/> <br>
<img src="https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white" />

> Current Status: 🚧 Early Development (Initial version in progress)

**automotion** — An experimental tool for automated batch exporting of Alight Motion project files via ADB.

| English         | Simplified Chinese            |
|-----------------|-------------------------------|
| English | [简体中文](./readme_zh-hans.md) |

## Introduction

`automotion` is a Rust-based ADB automation tool for batch-processing `.amproj` (Alight Motion project) files on an Android device via USB debugging.  
It solves the tedious manual workflow of importing, exporting, and rendering Alight Motion projects one by one, allowing users to automate the entire pipeline from push to pull.

With `automotion`, you only need to place `.amproj` files in the `input_projects/` directory and run a single command.  
The tool handles pushing files to the phone, importing into Alemon, navigating the UI, triggering export, monitoring render progress, and pulling back the finished `.mp4` videos.

## Features

* **Batch render** — Automatically process all `.amproj` files in `input_projects/` and export them as MP4 videos
* **Split** — Split a large `.amproj` project into individual element files
* **Split-render** — Split a project and then batch-render each element as a separate video
* **Dynamic UI interaction** — Finds UI elements by `resource-id` and `text` via `uiautomator` XML parsing (no hardcoded coordinates)
* **Render state detection** — Polls for `saveButton` appearance to detect render completion, supports up to 30-minute renders
* **Popup handling** — Automatically dismisses common popups (missing fonts, missing media, ad prompts)
* **Device protection** — Keeps screen awake during operation, restores on exit (including Ctrl+C)
* **Clean workflow** — Cleans up phone-side files after each project to prevent storage bloat

## How to Use

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clone the repository**:

   ```bash
   git clone https://github.com/Bli-AIk/automotion.git
   cd automotion
   ```

3. **Build and run**:

   ```bash
   cargo run
   ```

4. **Basic commands**:

   * Batch render all projects: `cargo run -- render`
   * Split a project into elements: `cargo run -- split <file.amproj>`
   * Split and render: `cargo run -- split-render <file.amproj>`

5. **Setup**:
   * Connect your Android device via USB with ADB debugging enabled
   * Install the Alemon app (package: `com.taffy.alemon`) on the device
   * Place `.amproj` files in the `./input_projects/` directory
   * Rendered videos will be saved to `./output_videos/`

## How to Build

### Prerequisites

* Rust 1.85 or later (edition 2024)
* ADB (Android Debug Bridge) installed and in PATH
* Android device connected via USB with debugging enabled

### Build Steps

1. **Clone the repository**:

   ```bash
   git clone https://github.com/Bli-AIk/automotion.git
   cd automotion
   ```

2. **Build the project**:

   ```bash
   cargo build --release
   ```

3. **Run tests**:

   ```bash
   cargo test
   ```

4. **Install globally** (optional):

   ```bash
   cargo install --path .
   ```

## Dependencies

This project uses the following crates:

| Crate                                           | Version | Description                          |
| ----------------------------------------------- | ------- | ------------------------------------ |
| [regex](https://crates.io/crates/regex)         | 1       | Regular expression parsing for UI XML |
| [clap](https://crates.io/crates/clap)           | 4       | Command-line argument parsing         |
| [zip](https://crates.io/crates/zip)             | 2       | ZIP archive reading/writing for amproj |
| [roxmltree](https://crates.io/crates/roxmltree) | 0.20    | XML parsing for amproj internals     |
| [chrono](https://crates.io/crates/chrono)       | 0.4     | Timestamp formatting for logs         |
| [ctrlc](https://crates.io/crates/ctrlc)         | 3       | Signal handling for clean shutdown    |

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
