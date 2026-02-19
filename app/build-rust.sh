#!/usr/bin/env bash
# ============================================================
# 构建 Rust .so 并复制到 Android jniLibs
# 前置要求：
#   rustup target add aarch64-linux-android armv7-linux-androideabi
#   cargo install cargo-ndk
#   设置 ANDROID_NDK_HOME 环境变量
# ============================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
JNILIBS_DIR="$SCRIPT_DIR/app/src/main/jniLibs"

echo "=== 构建 automotion-core Android .so ==="

cd "$PROJECT_ROOT"

# 交叉编译为 Android 目标架构
cargo ndk \
    -t arm64-v8a \
    -t armeabi-v7a \
    -o "$JNILIBS_DIR" \
    build --release -p automotion-core

echo "=== .so 文件已输出到 $JNILIBS_DIR ==="
ls -la "$JNILIBS_DIR"/*/

# 生成 UniFFI Kotlin 绑定
echo "=== 生成 UniFFI Kotlin 绑定 ==="
BINDINGS_DIR="$SCRIPT_DIR/app/src/main/java"

cargo run -p automotion-core --features uniffi/cli -- \
    generate --library "$PROJECT_ROOT/target/aarch64-linux-android/release/libautomotion_core.so" \
    --language kotlin \
    --out-dir "$BINDINGS_DIR"

echo "=== 构建完成 ==="
