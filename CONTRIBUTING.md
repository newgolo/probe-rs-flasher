# Contributing

Thanks for your interest in improving Probe-rs Flasher! This document explains how to build,
test and submit changes.

## Getting started

The project is a member of the [probe-rs](https://github.com/probe-rs/probe-rs) workspace.
The easiest way to get a working checkout is:

```bash
./scripts/bootstrap.sh --debug     # prepares .build/probe-rs and builds a debug binary
```

Alternatively, clone probe-rs at the revision in `.probe-rs-rev`, apply
`patches/probe-rs-workspace.patch` and copy this repository into the workspace as
`probe-rs-flasher` (see the README for the exact commands).

Requirements: Rust 1.89+, plus the system packages needed by `eframe` and `libudev`
(on Debian/Ubuntu: `pkg-config libudev-dev libxkbcommon-dev libwayland-dev`).

## Development workflow

```bash
cargo run -p probe-rs-flasher --release            # run the GUI
cargo run -p probe-rs-flasher --release -- fw.bin  # open a firmware file directly
cargo test  -p probe-rs-flasher                    # unit tests
cargo clippy -p probe-rs-flasher --all-targets     # lints
cargo fmt -p probe-rs-flasher                      # formatting
```

Useful environment variables during development:

| Variable | Purpose |
| --- | --- |
| `PROBE_RS_FLASHER_SIZE=1280x800` | Initial window size |
| `PROBE_RS_FLASHER_SHOT=/tmp/ui.ppm` | Render 30 frames and dump a PPM screenshot |
| `PROBE_RS_FLASHER_TARGETS=/tmp/targets` | Use a scratch directory for custom chips |

## Code layout

- `src/app.rs` — UI only; keep blocking work out of it.
- `src/worker.rs` — background thread and all probe-rs calls; communicate via the channel
  (`Command` / `Event`).
- `src/i18n.rs` — UI strings. Source strings are Chinese; add the English translation to the
  `en_for` table and wrap user-visible text with `t(...)` or `tt!(...)`.

## Localization

- English is the default language.
- Every user-visible string must go through `t("中文")` or `tt!("模板 {}", arg)` so the
  language toggle applies immediately.
- When adding a string, add the corresponding English entry in `src/i18n.rs`; the fallback
  keeps the Chinese text so the app never shows a missing translation marker.

## Commit messages

Please use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add language toggle to the title bar
fix: keep the selected chip after importing a pack
docs: translate README to English
```

## Pull requests

1. Fork the repository and create a topic branch.
2. Keep changes focused; describe the motivation in the PR description.
3. Make sure `cargo fmt`, `cargo clippy` and `cargo test` pass.
4. If you touched flashing logic, mention the probe/chip you tested with.

## License

By contributing you agree that your contributions are dual licensed under MIT OR Apache-2.0,
matching the rest of the project.
