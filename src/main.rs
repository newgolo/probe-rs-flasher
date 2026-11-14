//! Probe-rs Flasher —— 通用 MCU 固件下载工具。
//!
//! 基于 probe-rs 的图形化固件烧录应用：选择调试器与芯片型号后，
//! 可进行烧录 / 校验 / 擦除 / 读取 / 复位，以及任意地址的内存读写。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod chips;
mod custom;
mod firmware;
mod i18n;
mod theme;
mod worker;

use app::FlasherApp;

fn main() -> eframe::Result<()> {
    // 读取上次保存的界面语言（默认英文）。
    i18n::set_lang(i18n::load_persisted());

    // 允许通过环境变量覆盖初始窗口尺寸，例如 PROBE_RS_FLASHER_SIZE=1280x800
    let size = std::env::var("PROBE_RS_FLASHER_SIZE")
        .ok()
        .and_then(|value| {
            let (w, h) = value.split_once('x')?;
            Some([w.trim().parse::<f32>().ok()?, h.trim().parse::<f32>().ok()?])
        })
        .unwrap_or([1200.0, 900.0]);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Probe-rs Flasher")
            .with_inner_size(size)
            .with_min_inner_size([1000.0, 680.0])
            .with_decorations(false)
            .with_app_id("probe-rs-flasher"),
        ..Default::default()
    };

    // 支持在命令行中直接指定要打开的固件：probe-rs-flasher firmware.bin
    let initial_file = std::env::args_os().nth(1).map(std::path::PathBuf::from);

    eframe::run_native(
        "Probe-rs Flasher",
        options,
        Box::new(move |cc| Ok(Box::new(FlasherApp::new(cc, initial_file.clone())))),
    )
}
