#!/usr/bin/env bash
# ============================================================
# 构建 Rust .so 并复制到 Android jniLibs + 生成 UniFFI 绑定
# 前置要求：
#   rustup target add aarch64-linux-android armv7-linux-androideabi
#   cargo install cargo-ndk
#   设置 ANDROID_NDK_HOME 环境变量
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
JNILIBS_DIR="$SCRIPT_DIR/app/src/main/jniLibs"
BINDINGS_DIR="$SCRIPT_DIR/app/src/main/java"

# 自动检测 NDK 路径
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
    NDK_DIR="$HOME/Android/Sdk/ndk"
    if [ -d "$NDK_DIR" ]; then
        export ANDROID_NDK_HOME="$NDK_DIR/$(ls "$NDK_DIR" | sort -V | tail -1)"
        echo "自动检测 NDK: $ANDROID_NDK_HOME"
    else
        echo "错误: 未找到 Android NDK，请设置 ANDROID_NDK_HOME" >&2
        exit 1
    fi
fi

echo "=== 构建 automotion-core Android .so ==="
cd "$PROJECT_ROOT"

# 交叉编译为 Android 目标架构
cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -o "$JNILIBS_DIR" \
    build --release -p automotion-core

echo "=== .so 文件已输出到 $JNILIBS_DIR ==="
ls -lh "$JNILIBS_DIR"/*/

# 生成 UniFFI Kotlin 绑定
echo "=== 生成 UniFFI Kotlin 绑定 ==="
cargo run -p automotion-core --bin uniffi-bindgen -- \
    generate \
    --library "$PROJECT_ROOT/target/aarch64-linux-android/release/libautomotion_core.so" \
    --language kotlin \
    --out-dir "$BINDINGS_DIR" \
    --no-format

echo "=== 构建完成 ==="
echo "绑定文件: $BINDINGS_DIR/uniffi/automotion_core/automotion_core.kt"
