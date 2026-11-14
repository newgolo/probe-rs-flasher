#!/usr/bin/env bash
# 在全新的 probe-rs 工作区中编译本仓库（可复现构建，供 CI 与首次搭建使用）。
#
# 流程：
#   1. 克隆 .probe-rs-rev 中固定版本的 probe-rs；
#   2. 应用 patches/probe-rs-workspace.patch（工作区成员 + target-gen 可选特性）；
#   3. 把本仓库复制进工作区，执行 build-linux.sh（其余参数原样转发）。
#
# 用法:
#   ./scripts/bootstrap.sh              release 构建
#   ./scripts/bootstrap.sh --no-hid     透传给 build-linux.sh
#   PROBE_RS_BUILD_DIR=/tmp/work ./scripts/bootstrap.sh   自定义工作区目录
#   BOOTSTRAP_PREPARE_ONLY=1 ./scripts/bootstrap.sh       只准备环境并打印工作区路径
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
PIN="$(tr -d '[:space:]' < "$REPO_ROOT/.probe-rs-rev")"
BUILD_DIR="${PROBE_RS_BUILD_DIR:-$REPO_ROOT/.build}"
WORKSPACE="$BUILD_DIR/probe-rs"

if [ -z "$PIN" ]; then
    echo "缺少 $REPO_ROOT/.probe-rs-rev（probe-rs 版本）" >&2
    exit 1
fi

mkdir -p "$BUILD_DIR"

if [ ! -d "$WORKSPACE/.git" ]; then
    echo "==> 克隆 probe-rs 到 $WORKSPACE"
    git clone --filter=blob:none https://github.com/probe-rs/probe-rs.git "$WORKSPACE"
fi

echo "==> 检出 probe-rs $PIN"
if ! git -C "$WORKSPACE" checkout --force --quiet "$PIN" 2>/dev/null; then
    git -C "$WORKSPACE" fetch --quiet origin "$PIN"
    git -C "$WORKSPACE" checkout --force --quiet FETCH_HEAD
fi

echo "==> 应用工作区补丁"
git -C "$WORKSPACE" checkout --force --quiet -- Cargo.toml target-gen
git -C "$WORKSPACE" apply "$REPO_ROOT/patches/probe-rs-workspace.patch"

echo "==> 复制 probe-rs-flasher 源码"
rm -rf "$WORKSPACE/probe-rs-flasher"
mkdir -p "$WORKSPACE/probe-rs-flasher"
tar -C "$REPO_ROOT" \
    --exclude=./.git --exclude=./.build --exclude=./dist --exclude=./target \
    -cf - . | tar -C "$WORKSPACE/probe-rs-flasher" -xf -

if [ "${BOOTSTRAP_PREPARE_ONLY:-0}" = "1" ]; then
    echo "$WORKSPACE"
    exit 0
fi

echo "==> 开始构建"
exec bash "$WORKSPACE/probe-rs-flasher/build-linux.sh" "$@"
