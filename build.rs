//! 构建脚本：为 Windows 可执行文件嵌入图标。
//!
//! Linux / macOS 上不做任何事；Windows（含交叉编译）下调用 `windres` 把
//! `assets/app.rc` 编译成目标文件并链接进可执行文件，这样资源管理器、
//! 任务栏和快捷方式都能显示应用图标。
//!
//! 找不到 `windres` 时只打印警告，不影响构建（此时窗口图标仍然有效，
//! 由 `src/icon.rs` 在运行时通过 `ViewportBuilder::with_icon` 设置）。

use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-env-changed=WINDRES");

    let target = std::env::var("TARGET").unwrap_or_default();
    if !target.contains("windows") {
        return;
    }

    let default_windres = if target.contains("gnu") {
        "x86_64-w64-mingw32-windres"
    } else {
        "windres"
    };
    let windres = std::env::var("WINDRES")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_windres.to_string());

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let obj = out_dir.join("app-icon.o");

    let output = Command::new(&windres)
        .arg("assets/app.rc")
        .arg("-O")
        .arg("coff")
        .arg("-o")
        .arg(&obj)
        .output();

    match output {
        Ok(output) if output.status.success() => {
            println!("cargo:rustc-link-arg-bins={}", obj.display());
        }
        Ok(output) => {
            println!(
                "cargo:warning=windres failed, building without an executable icon: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Err(err) => {
            println!(
                "cargo:warning=could not run {windres} ({err}); building without an executable icon"
            );
        }
    }
}
