#!/usr/bin/env bash
# 编译 Linux (x86_64) 版本。
#
# 依赖策略：
#   * Rust / C 依赖全部静态链接（不使用 libudev 时无额外运行库依赖）；
#   * X11 / Wayland / OpenGL / EGL 由 winit / glutin 在运行时 dlopen 加载，
#     因此不会出现在二进制文件的 NEEDED 列表里（桌面系统本来就自带）；
#   * 最终只依赖 glibc 基础库（libc / libm / libgcc_s / ld-linux）。
#
# 用法:
#   ./build-linux.sh             release 构建（完整特性，含 CMSIS-DAP v1/HID）
#   ./build-linux.sh --no-hid    不编译 CMSIS-DAP v1(HID) 后端（体积略小）
#   ./build-linux.sh --debug     调试构建
#   ./build-linux.sh --help
#
# 产物: dist/probe-rs-flasher        （--no-hid 时为 dist/probe-rs-flasher-nohid）

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(dirname "$SCRIPT_DIR")"
TARGET="x86_64-unknown-linux-gnu"

WITH_HID=1
PROFILE="release"

for arg in "$@"; do
    case "$arg" in
        --no-hid) WITH_HID=0 ;;
        --debug) PROFILE="debug" ;;
        -h|--help)
            awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
            exit 0
            ;;
        *)
            echo "未知参数: $arg（使用 --help 查看用法）" >&2
            exit 1
            ;;
    esac
done

# --- 确保 cargo / rustc 在 PATH 中（脚本在非交互 shell 中运行时常见） ----------
if ! command -v cargo >/dev/null 2>&1; then
    for candidate in "${CARGO_HOME:-$HOME/.cargo}/bin" "$HOME/.cargo/bin" /usr/local/cargo/bin; do
        if [ -x "$candidate/cargo" ]; then
            export PATH="$candidate:$PATH"
            break
        fi
    done
fi
if ! command -v cargo >/dev/null 2>&1; then
    for candidate in "$HOME"/.rustup/toolchains/*/bin; do
        if [ -x "$candidate/cargo" ]; then
            export PATH="$candidate:$PATH"
            break
        fi
    done
fi
if ! command -v cargo >/dev/null 2>&1; then
    echo "未找到 cargo，请先安装 Rust: https://rustup.rs" >&2
    exit 1
fi


if [ "$WITH_HID" -eq 1 ]; then
    FEATURES=()
    OUT_NAME="probe-rs-flasher"
else
    # 不编译 CMSIS-DAP v1(HID) 后端（hidapi），体积略小。
    # CMSIS-DAP v2 / J-Link / ST-Link / FTDI 等调试器不受影响。
    FEATURES=(--no-default-features --features builtin-targets,builtin-formats)
    OUT_NAME="probe-rs-flasher-nohid"
fi

# 调试版单独命名，避免覆盖 release 产物
if [ "$PROFILE" = "debug" ]; then
    OUT_NAME="$OUT_NAME-debug"
fi

if [ "$PROFILE" = "release" ]; then
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS="-C strip=symbols"
    BUILD_ARGS=(--release)
else
    export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS=""
    BUILD_ARGS=()
fi

LIBDIR="$(rustc --print target-libdir --target "$TARGET" 2>/dev/null || true)"
if [ -z "$LIBDIR" ] || [ ! -d "$LIBDIR" ]; then
    echo "缺少 Rust 标准库 target: $TARGET" >&2
    echo "请执行: rustup target add $TARGET" >&2
    exit 1
fi

cd "$WORKSPACE_DIR"
echo "==> Rust: $(cargo --version 2>/dev/null)"
echo "==> 编译 probe-rs-flasher ($TARGET, $PROFILE, hid=$WITH_HID)"
cargo build -p probe-rs-flasher --target "$TARGET" "${BUILD_ARGS[@]}" "${FEATURES[@]}"

SRC="target/$TARGET/$PROFILE/probe-rs-flasher"
OUT_DIR="$SCRIPT_DIR/dist"
mkdir -p "$OUT_DIR"
cp -f "$SRC" "$OUT_DIR/$OUT_NAME"

echo
echo "==> 产物: $OUT_DIR/$OUT_NAME"
ls -lh "$OUT_DIR/$OUT_NAME"
echo
echo "==> 动态库依赖 (NEEDED):"
if readelf -d "$OUT_DIR/$OUT_NAME" 2>/dev/null | grep -q NEEDED; then
    readelf -d "$OUT_DIR/$OUT_NAME" | awk '/NEEDED/ { print "    " $NF }' | tr -d '[]'
else
    echo "    （无，完全静态链接）"
fi
echo
echo "提示: X11 / Wayland / OpenGL 等桌面库由程序在运行时自动加载，无需额外安装。"
