#!/usr/bin/env bash
# 交叉编译 Windows x64 版本 (x86_64-pc-windows-gnu)。
#
# 依赖策略：
#   * 使用 MinGW-w64 静态链接（Rust 标准库 + libgcc + libwinpthread 全部内置），
#     因此不需要随程序分发任何 MinGW DLL；
#   * 最终只依赖 Windows 系统自带的 DLL（kernel32 / user32 / msvcrt / opengl32 / ...）。
#
# 前置条件:
#   1. rustup（脚本会自动安装 x86_64-pc-windows-gnu target）；
#   2. 自行安装的 MinGW-w64 工具链。脚本按以下顺序查找 x86_64-w64-mingw32-gcc：
#        a) $MINGW_ROOT/usr/bin/                （自定义前缀，例如解压的 deb 包）
#        b) $MINGW_ROOT/bin/                    （官方压缩包解压后的目录）
#        c) 系统 PATH                            （apt / pacman / 官方安装包等）
#      例如 Ubuntu/WSL: sudo apt install gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64
#
# 用法:
#   ./build-windows.sh             release 构建
#   ./build-windows.sh --debug     调试构建（产物名带 -debug）
#   ./build-windows.sh --help
#
# 产物: dist/probe-rs-flasher.exe

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(dirname "$SCRIPT_DIR")"
TARGET="x86_64-pc-windows-gnu"
PROFILE="release"

for arg in "$@"; do
    case "$arg" in
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

# --- 查找 MinGW-w64 -----------------------------------------------------------
if [ -n "${MINGW_ROOT:-}" ]; then
    for candidate in "$MINGW_ROOT/usr/bin" "$MINGW_ROOT/bin"; do
        if [ -x "$candidate/x86_64-w64-mingw32-gcc" ]; then
            export PATH="$candidate:$PATH"
            break
        fi
    done
fi

if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    echo "未找到 x86_64-w64-mingw32-gcc（MinGW-w64）。" >&2
    echo >&2
    echo "请先安装 MinGW-w64，例如：" >&2
    echo "  Ubuntu/WSL : sudo apt install gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64" >&2
    echo "  Arch       : sudo pacman -S mingw-w64-gcc" >&2
    echo "  官方压缩包 : 解压后 export MINGW_ROOT=/path/to/mingw64" >&2
    exit 1
fi

export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
export CC_x86_64_pc_windows_gnu=x86_64-w64-mingw32-gcc
export AR_x86_64_pc_windows_gnu="$(command -v x86_64-w64-mingw32-ar || echo x86_64-w64-mingw32-ar)"

if [ "$PROFILE" = "release" ]; then
    # strip 掉符号，得到较小的单文件产物
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C strip=symbols"
    BUILD_ARGS=(--release)
else
    export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS=""
    BUILD_ARGS=()
fi

if command -v rustup >/dev/null 2>&1; then
    rustup target add "$TARGET" >/dev/null 2>&1 || true
fi

LIBDIR="$(rustc --print target-libdir --target "$TARGET" 2>/dev/null || true)"
if [ -z "$LIBDIR" ] || [ ! -d "$LIBDIR" ]; then
    echo "缺少 Rust 标准库 target: $TARGET" >&2
    echo "请执行: rustup target add $TARGET" >&2
    exit 1
fi

cd "$WORKSPACE_DIR"
echo "==> Rust: $(cargo --version 2>/dev/null)"
echo "==> 编译器: $(command -v x86_64-w64-mingw32-gcc) ($(x86_64-w64-mingw32-gcc -dumpversion 2>/dev/null || echo '?'))"
echo "==> 编译 probe-rs-flasher ($TARGET, $PROFILE)"
cargo build -p probe-rs-flasher --target "$TARGET" "${BUILD_ARGS[@]}"

SRC="target/$TARGET/$PROFILE/probe-rs-flasher.exe"
OUT_DIR="$SCRIPT_DIR/dist"
OUT_NAME="probe-rs-flasher"
if [ "$PROFILE" = "debug" ]; then
    OUT_NAME="$OUT_NAME-debug"
fi
mkdir -p "$OUT_DIR"
cp -f "$SRC" "$OUT_DIR/$OUT_NAME.exe"

echo
echo "==> 产物: $OUT_DIR/$OUT_NAME.exe"
ls -lh "$OUT_DIR/$OUT_NAME.exe"

if command -v x86_64-w64-mingw32-objdump >/dev/null 2>&1; then
    echo
    echo "==> 依赖的 DLL:"
    x86_64-w64-mingw32-objdump -p "$OUT_DIR/$OUT_NAME.exe" \
        | awk '/DLL Name/ { print $3 }' | sort -u \
        | awk '{ printf "    %s\n", $0; if ($0 ~ /^(libgcc|libwinpthread|libstdc\+\+|zlib)/) bad = 1 }
               END { if (bad) { print "\n警告: 存在非系统 DLL 依赖，请检查 MinGW 静态链接设置。"; exit 1 } }'
fi
