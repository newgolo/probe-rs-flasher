<p align="center">
  <img src="assets/icon-128.png" width="96" height="96" alt="Probe-rs Flasher 图标">
</p>

# Probe-rs Flasher

[English](README.md) · **简体中文**

[![CI](https://github.com/newgolo/probe-rs-flasher/actions/workflows/ci.yml/badge.svg)](https://github.com/newgolo/probe-rs-flasher/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#许可证)
[![Rust](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://www.rust-lang.org)

基于 [probe-rs](https://probe.rs) 的跨平台 MCU 固件下载工具（GUI）。选择调试器与芯片型号后，
拖入固件即可烧录。软件内置 **英文 / 中文** 一键切换（标题栏右上角），**默认英文**。

支持 probe-rs 内置的全部目标芯片（4000+ 型号）以及 probe-rs 支持的各种调试器
（DAPLink、J-Link、ST-Link、CMSIS-DAP、ESP JTAG、FTDI 等）。

---

## 功能

- **连接设置**：调试器选择、SWD / JTAG、1 MHz ~ 50 MHz 速率、复位方式
  （默认 / 连接时复位 / 硬件复位）。
- **芯片选择**：厂家 → 系列 → 型号三级联动，支持关键字搜索（如 `STM32`、`F103VC`）。
- **自定义芯片**：导入 CMSIS-Pack（`.pack` / 解压目录 / `.pdsc`）或 probe-rs 目标描述
  YAML，由 `target-gen` 生成并持久化，与内置芯片一起出现在选择列表中。
- **固件加载**：拖拽或点击选择 `.bin` / `.hex` / `.elf` / `.uf2`，自动解析、计算 CRC32，
  并以十六进制视图（地址 + 16 字节 HEX + ASCII）展示。
- **烧录操作**：烧录（可选整片擦除、烧录后自动校验）、校验、擦除、读取、复位、
  任意地址内存读写，以及尽力中断长任务的「终止」。
- **自动化选项**：整片擦除、自动烧录（连接后自动烧录已加载固件）、自动连接。
- **日志面板**：带时间戳的操作日志，实时显示进度条与百分比。
- **国际化**：运行时切换英文 / 中文，选择结果保存在用户配置目录，下次启动自动恢复。
- **单文件产物**：每个平台一个可执行文件，不需要额外的运行库。

## 下载

预编译的单文件二进制发布在 [Releases](https://github.com/newgolo/probe-rs-flasher/releases)：

| 平台 | 文件 |
| --- | --- |
| Windows 10/11 x64 | `probe-rs-flasher.exe` |
| Linux x86_64 | `probe-rs-flasher` |

每个版本附带 SHA-256 校验文件；也可以参考下文自行编译。

## 环境要求

| 平台 | 支持情况 | 说明 |
| --- | --- | --- |
| Windows | Windows 10/11 x64 | 使用 WinUSB / HID，CMSIS-DAP v2 免装驱动 |
| Linux | x86_64 | 需要 X11 或 Wayland + OpenGL（运行时加载） |
| macOS | 未测试 | 可用 `cargo` 编译，参考 probe-rs 平台说明 |

在 Linux 上使用真实硬件需要安装 udev 规则（或具有设备访问权限），详见
<https://probe.rs/docs/getting-started/probe-setup/>。

## 快速上手

1. 用 USB 连接调试器与目标板；
2. 在「设备」中选择调试器（默认自动选择第一个；勾选「自动连接」时会自动连接）；
3. 选择「厂家 / 系列 / 型号」，或在搜索框输入型号关键字（内置库没有时可点击
   「导入芯片…」添加）；
4. 将固件文件拖入「固件」区域（或点击选择文件）；
5. 点击「烧录」。勾选「整片擦除」会先执行芯片级全片擦除，烧录完成后自动校验。

> `.bin` 文件默认使用目标芯片的 Flash 起始地址（如 STM32 为 `0x08000000`），
> 连接后会自动按实际目标修正；`.hex` / `.elf` / `.uf2` 使用文件自带的地址信息。

## 编译

### 方式一：在 probe-rs 工作区内编译（推荐）

本 crate 是 [probe-rs](https://github.com/probe-rs/probe-rs) 工作区的成员，复用其 crates
以及少量本地补丁（`target-gen` 的可选特性）。仓库自带脚本可一键搭建环境：

```bash
./scripts/bootstrap.sh            # 克隆 probe-rs、应用补丁、release 构建
./scripts/bootstrap.sh --no-hid   # 其余参数会转发给 build-linux.sh
```

脚本会克隆 `.probe-rs-rev` 中固定版本的 probe-rs，应用
`patches/probe-rs-workspace.patch`，把本仓库复制进工作区并执行 `build-linux.sh`。

### 方式二：已有 probe-rs 源码

```bash
git clone https://github.com/probe-rs/probe-rs.git
cd probe-rs
git checkout "$(cat /path/to/probe-rs-flasher/.probe-rs-rev)"
git apply /path/to/probe-rs-flasher/patches/probe-rs-workspace.patch
cp -r /path/to/probe-rs-flasher ./probe-rs-flasher
./probe-rs-flasher/build-linux.sh
```

### 构建脚本

两个脚本都会把产物写入 `dist/`：

```bash
# Linux x86_64
./build-linux.sh              # release（完整特性，含 CMSIS-DAP v1 / HID）
./build-linux.sh --no-hid     # 不编译 HID 后端，体积略小
./build-linux.sh --debug      # 调试版

# Windows x64（在 Linux / WSL 上用 MinGW-w64 交叉编译）
./build-windows.sh            # 产物: dist/probe-rs-flasher.exe
```

脚本会自动把 `~/.cargo/bin`（或 `$CARGO_HOME/bin`）加入 `PATH`，因此在非交互 shell
（如 CI、IDE 终端）中也无需手动 export。

也可以直接使用 cargo：

```bash
cargo build -p probe-rs-flasher --release
cargo run   -p probe-rs-flasher --release -- firmware.bin
```

### 静态链接与依赖

| 平台 | 产物 | 依赖 |
| --- | --- | --- |
| Windows x64 | `dist/probe-rs-flasher.exe`（约 23 MB） | 仅 Windows 系统 DLL（kernel32、user32、msvcrt、opengl32、setupapi、winusb 等），**不需要任何 MinGW 运行库** |
| Linux x86_64 | `dist/probe-rs-flasher`（约 19 MB） | 仅 glibc 基础库（`libc`、`libm`、`libgcc_s`、`ld-linux`）；X11 / Wayland / OpenGL / EGL 由程序运行时 `dlopen` 加载，不出现在 `NEEDED` 列表中 |

说明：

- Linux 上使用静态 musl 也能编译，但 winit 通过 `dlopen` 加载 X11/Wayland，而完全静态的
  可执行文件不支持 `dlopen`，因此 Linux 版本采用 glibc 动态链接 + Rust/C 依赖静态化。
- CMSIS-DAP v1（HID）后端通过 `dlopen` 访问 libudev，不会增加硬依赖；不需要 HID 时可用
  `--no-hid` 略微减小体积。
- 编译期特性可通过 `--no-default-features --features ...` 控制：`builtin-targets`
  （内置芯片库）、`builtin-formats`（bin/hex/elf/uf2）、`cmsisdap-v1`（HID 调试器）。

### Windows 交叉编译环境

`build-windows.sh` 按以下顺序查找 `x86_64-w64-mingw32-gcc`：

1. `$MINGW_ROOT/usr/bin/`
2. `$MINGW_ROOT/bin/`
3. 系统 `PATH`

例如在 Ubuntu / WSL 中安装：

```bash
sudo apt install gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64
```

## 自定义芯片

内置芯片库里没有型号时，可通过「芯片选择 → 导入芯片…」自行添加：

| 输入 | 说明 |
| --- | --- |
| `.pack` / `.zip` | CMSIS-Pack 压缩包（推荐），从中提取 Flash 算法 |
| 目录 | 解压后的 Pack 目录（包含 `.pdsc` 与算法文件 `.FLM`） |
| `.pdsc` | 会扫描该文件所在目录以获取完整 Pack 内容 |
| `.yaml` / `.yml` | probe-rs / target-gen 格式的目标描述文件 |

工作方式：

1. 使用 `target-gen` 解析 CMSIS-Pack，得到内存映射与 Flash 算法；
2. 通过与 probe-rs 内置目标一致的 YAML 格式序列化；
3. 保存到用户配置目录，并在启动时加载到运行时 `Registry`：

   | 系统 | 目录 |
   | --- | --- |
   | Linux | `~/.config/probe-rs-flasher/targets` |
   | Windows | `%APPDATA%\probe-rs-flasher\targets` |
   | macOS | `~/Library/Application Support/probe-rs-flasher/targets` |

   便携使用可用环境变量 `PROBE_RS_FLASHER_TARGETS` 覆盖；
4. 连接目标时使用「内置 + 自定义」注册表，因此自定义芯片同样可以烧录、校验、擦除、读取。

导入的芯片统一归入「自定义芯片(导入)」分组并排在最前；「管理自定义」窗口可查看、
删除已导入的系列，或直接打开配置目录。删除后立即刷新，无需重启。

## 架构

| 文件 | 说明 |
| --- | --- |
| `src/main.rs` | 程序入口、窗口参数、启动语言 |
| `src/app.rs` | 界面布局与交互（面板、文件选择器、内存窗口、标题栏） |
| `src/worker.rs` | 后台线程：所有 probe-rs 操作，通过 channel 与 UI 通信 |
| `src/chips.rs` | 构建「厂家 / 系列 / 型号」数据库（含自定义芯片分组） |
| `src/custom.rs` | 自定义芯片：CMSIS-Pack / YAML 解析、持久化、加载与注册 |
| `src/firmware.rs` | `.bin` / `.hex` / `.elf` / `.uf2` 解析、CRC32、十六进制视图 |
| `src/theme.rs` | 配色、字体与自绘控件（分组框、图标按钮、虚线拖拽区） |
| `src/i18n.rs` | 轻量国际化（默认英文，中文作为 key） |
| `build-linux.sh` | Linux x86_64 构建脚本 |
| `build-windows.sh` | Windows x64 交叉编译脚本 |
| `scripts/bootstrap.sh` | 可复现的工作区搭建与构建 |
| `tools/gen_icon.py` | 生成应用图标（`assets/`） |
| `build.rs` | 通过 `windres` 把 `.ico` 嵌入 Windows 可执行文件 |

所有耗时操作都在后台线程执行，界面不会阻塞；「终止」通过原子标志与 probe-rs 的进度
回调配合实现。

## 开发辅助环境变量

| 变量 | 作用 |
| --- | --- |
| `PROBE_RS_FLASHER_SIZE=1200x900` | 覆盖初始窗口尺寸 |
| `PROBE_RS_FLASHER_SHOT=/tmp/ui.ppm` | 渲染 30 帧后输出 PPM 截图（便于 CI / 远程环境核对界面） |
| `PROBE_RS_FLASHER_TARGETS=/path/to/targets` | 覆盖自定义芯片目录 |

## 参与贡献

欢迎提交 Issue 与 Pull Request，详见 [CONTRIBUTING.md](CONTRIBUTING.md)。
提交前请运行 `cargo fmt` 与 `cargo clippy`。

## 许可证

与 probe-rs 保持一致，采用下列任一许可证：

- Apache License, Version 2.0（[LICENSE-APACHE](LICENSE-APACHE)）
- MIT License（[LICENSE-MIT](LICENSE-MIT)）

除非你显式声明，否则有意提交以纳入本项目的任何贡献都将按上述双许可授权。
