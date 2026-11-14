#!/usr/bin/env bash
# CI 入口：准备 probe-rs 工作区，运行测试并做一次 release 构建。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="${PROBE_RS_BUILD_DIR:-$REPO_ROOT/.build}"
WORKSPACE="$BUILD_DIR/probe-rs"

BOOTSTRAP_PREPARE_ONLY=1 PROBE_RS_BUILD_DIR="$BUILD_DIR" \
    bash "$SCRIPT_DIR/bootstrap.sh" >/dev/null

cd "$WORKSPACE"
echo "==> cargo test -p probe-rs-flasher"
cargo test -p probe-rs-flasher

echo "==> release 构建"
bash "$WORKSPACE/probe-rs-flasher/build-linux.sh"
