<p align="center">
  <img src="assets/icon-128.png" width="96" height="96" alt="Probe-rs Flasher icon">
</p>

# Probe-rs Flasher

**English** · [简体中文](README.zh-CN.md)

[![CI](https://github.com/newgolo/probe-rs-flasher/actions/workflows/ci.yml/badge.svg)](https://github.com/newgolo/probe-rs-flasher/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)
[![Rust](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://www.rust-lang.org)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](#platform-support)

A cross-platform, single-binary GUI firmware flasher for microcontrollers, powered by
[probe-rs](https://probe.rs). Pick a debug probe, pick a chip, drop a firmware file, and flash.

The interface ships in **English by default**, with a one-click English / 中文 switch in the
title bar.

---

## Features

- **Probe & connection** — probe selection, SWD / JTAG, 1–50 MHz clock, reset modes
  (default / under-reset / hardware reset).
- **Chip database** — 4000+ built-in target definitions grouped by vendor → series → chip,
  with keyword search (e.g. `STM32`, `F103VC`).
- **Custom chips** — import a CMSIS-Pack (`.pack`, unpacked directory or `.pdsc`) or a probe-rs
  target YAML; definitions are generated with `target-gen`, persisted locally and merged into
  the runtime registry.
- **Firmware loading** — drag & drop or pick `.bin` / `.hex` / `.elf` / `.uf2`, automatic
  parsing, CRC32, and a hex viewer (address + 16-byte hex + ASCII).
- **Operations** — flash (optionally chip-erase and verify), verify, erase, read back, reset,
  arbitrary memory read/write, and a best-effort **Abort** for long-running jobs.
- **Automation** — chip erase, auto-flash after connect, periodic auto-connect.
- **Log panel** — timestamped operation log with a live progress bar.
- **Localization** — runtime English / Chinese switch; the choice is stored in the user
  configuration directory and restored on the next start.
- **Portable output** — one executable per platform with no extra runtime libraries.

## Download

Prebuilt single-file binaries are published on the
[releases page](https://github.com/newgolo/probe-rs-flasher/releases):

| Platform | File |
| --- | --- |
| Windows 10/11 x64 | `probe-rs-flasher.exe` |
| Linux x86_64 | `probe-rs-flasher` |

Each release includes SHA-256 checksums. To build from source instead, see
[Building](#building).

## Requirements

| Platform | Supported | Notes |
| --- | --- | --- |
| Windows | Windows 10/11 x64 | uses WinUSB / HID, no driver install for CMSIS-DAP v2 |
| Linux | x86_64 | X11 or Wayland + OpenGL (loaded at runtime) |
| macOS | untested | builds with `cargo`, see the probe-rs platform notes |

Running against real hardware on Linux requires udev rules (or sufficient device permissions);
see <https://probe.rs/docs/getting-started/probe-setup/>.

## Quick start

1. Connect your debug probe and target board.
2. Select the probe, interface, clock and reset mode in **Connection**.
3. Choose vendor → series → chip, or search for the part number. If your chip is missing, use
   **Chip → Import chip…**.
4. Drop the firmware file onto the **Firmware** panel (or click to browse).
5. Click **Flash**. Enable *Chip erase* for a full chip erase first; verification runs after
   flashing automatically.

> For `.bin` files the target flash base address is used (e.g. `0x08000000` on STM32) and is
> corrected automatically once a target is connected. `.hex` / `.elf` / `.uf2` files carry
> their own addresses.

## Building

### Option A — build inside the probe-rs workspace

This crate is a member of the [probe-rs](https://github.com/probe-rs/probe-rs) workspace and
reuses its crates plus a few local patches (optional `target-gen` features). The helper script
sets everything up from scratch:

```bash
./scripts/bootstrap.sh            # clone probe-rs, apply patches, build release
./scripts/bootstrap.sh --no-hid   # extra arguments are forwarded to build-linux.sh
```

`scripts/bootstrap.sh` clones probe-rs at the revision pinned in `.probe-rs-rev`, applies
`patches/probe-rs-workspace.patch`, copies this repository into the workspace and runs
`build-linux.sh`.

### Option B — you already have a probe-rs checkout

```bash
git clone https://github.com/probe-rs/probe-rs.git
cd probe-rs
git checkout "$(cat /path/to/probe-rs-flasher/.probe-rs-rev)"
git apply /path/to/probe-rs-flasher/patches/probe-rs-workspace.patch
cp -r /path/to/probe-rs-flasher ./probe-rs-flasher
./probe-rs-flasher/build-linux.sh
```

### Build scripts

Both scripts write into `dist/`:

```bash
# Linux x86_64
./build-linux.sh              # release, full feature set (incl. CMSIS-DAP v1 / HID)
./build-linux.sh --no-hid     # smaller binary without the HID backend
./build-linux.sh --debug      # debug build

# Windows x64 (cross-compiled from Linux/WSL with MinGW-w64)
./build-windows.sh            # dist/probe-rs-flasher.exe
```

The scripts also add `~/.cargo/bin` (or `$CARGO_HOME/bin`) to `PATH`, so they work from
non-interactive shells such as CI or IDE terminals.

Plain cargo works too:

```bash
cargo build -p probe-rs-flasher --release
cargo run   -p probe-rs-flasher --release -- firmware.bin
```

### Static linking & runtime dependencies

| Platform | Artifact | Runtime dependencies |
| --- | --- | --- |
| Windows x64 | `dist/probe-rs-flasher.exe` (~23 MB) | Windows system DLLs only (kernel32, user32, msvcrt, opengl32, setupapi, winusb, …); no MinGW runtime required |
| Linux x86_64 | `dist/probe-rs-flasher` (~19 MB) | glibc only (`libc`, `libm`, `libgcc_s`, `ld-linux`); X11 / Wayland / OpenGL / EGL are loaded at runtime via `dlopen` and are not hard `NEEDED` entries |

Notes:

- A static musl build compiles, but winit loads X11/Wayland with `dlopen`, which does not work
  in fully static executables — hence glibc dynamic linking with all Rust/C dependencies
  statically linked.
- The CMSIS-DAP v1 (HID) backend reaches libudev through `dlopen`, so it does not add a hard
  dependency. Use `--no-hid` if you do not need it.
- Compile-time features: `builtin-targets` (chip database), `builtin-formats`
  (bin/hex/elf/uf2) and `cmsisdap-v1` (HID probes).

### Windows cross-compilation

`build-windows.sh` looks for `x86_64-w64-mingw32-gcc` in `$MINGW_ROOT/usr/bin`,
`$MINGW_ROOT/bin` and `PATH`. On Ubuntu/WSL:

```bash
sudo apt install gcc-mingw-w64-x86-64 binutils-mingw-w64-x86-64
```

## Custom chip support

If a part is not in the built-in database, add it from **Chip → Import chip…**:

| Input | Description |
| --- | --- |
| `.pack` / `.zip` | CMSIS-Pack archive (recommended); flash algorithms are extracted |
| Directory | Unpacked pack directory containing the `.pdsc` and algorithm files (`.FLM`) |
| `.pdsc` | The containing directory is scanned for the full pack contents |
| `.yaml` / `.yml` | probe-rs / target-gen target description |

How it works:

1. `target-gen` parses the CMSIS-Pack into memory maps and flash algorithms.
2. Definitions are serialized to the same YAML format as probe-rs built-in targets.
3. They are stored in the user configuration directory and loaded into the runtime `Registry`:

   | OS | Directory |
   | --- | --- |
   | Linux | `~/.config/probe-rs-flasher/targets` |
   | Windows | `%APPDATA%\probe-rs-flasher\targets` |
   | macOS | `~/Library/Application Support/probe-rs-flasher/targets` |

   Override with `PROBE_RS_FLASHER_TARGETS` for portable setups.
4. Attaching a target uses a registry that contains built-in **and** custom chips, so custom
   parts can be flashed, verified, erased and read like any other.

Imported parts are grouped under **Custom (imported)** and sorted first. The
**Manage custom** window lists and deletes them, or opens the configuration directory.
The registry reloads immediately — no restart needed.

## Architecture

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Entry point, window options, startup language |
| `src/app.rs` | UI layout and interaction (panels, file picker, memory window, title bar) |
| `src/worker.rs` | Background worker thread; all probe-rs operations, events over a channel |
| `src/chips.rs` | Builds the vendor → series → chip database, including custom groups |
| `src/custom.rs` | Custom chip pipeline: CMSIS-Pack/YAML parsing, persistence, loading |
| `src/firmware.rs` | `.bin` / `.hex` / `.elf` / `.uf2` parsing, CRC32, hex dump helpers |
| `src/theme.rs` | Colors, fonts and custom widgets (group boxes, icon buttons, dashed drop zone) |
| `src/i18n.rs` | Minimal localization layer (English default, Chinese keys) |
| `build-linux.sh` | Linux x86_64 build script |
| `build-windows.sh` | Windows x64 cross-compilation script |
| `scripts/bootstrap.sh` | Reproducible workspace setup and build |
| `tools/gen_icon.py` | Generates the application icon set (`assets/`) |
| `build.rs` | Embeds the `.ico` into Windows executables via `windres` |

All blocking work happens on the worker thread; the UI never stalls. **Abort** combines an
atomic flag with probe-rs progress callbacks.

## Environment variables

| Variable | Purpose |
| --- | --- |
| `PROBE_RS_FLASHER_SIZE=1200x900` | Override the initial window size |
| `PROBE_RS_FLASHER_SHOT=/tmp/ui.ppm` | Render 30 frames, then write a PPM screenshot (CI / remote review) |
| `PROBE_RS_FLASHER_TARGETS=/path/to/targets` | Override the custom chip directory |

## Contributing

Issues and pull requests are welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).
Please run `cargo fmt` and `cargo clippy` before submitting.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. This matches the licensing of probe-rs.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion
in this project shall be dual licensed as above, without any additional terms or conditions.
