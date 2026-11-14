//! 应用图标：64×64 RGBA 数据在编译期嵌入，运行时无需图片解码依赖。
//!
//! 图标源文件位于 `assets/`，由 `tools/gen_icon.py` 生成：蓝色渐变圆角方块
//! 加白色下载箭头，表示「把固件写入芯片」。

const ICON_RGBA: &[u8] = include_bytes!("../assets/icon-64.rgba");
const ICON_SIZE: u32 = 64;

/// 窗口 / 任务栏图标。
pub fn window_icon() -> egui::IconData {
    egui::IconData {
        rgba: ICON_RGBA.to_vec(),
        width: ICON_SIZE,
        height: ICON_SIZE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_icon_has_expected_size() {
        assert_eq!(
            ICON_RGBA.len(),
            (ICON_SIZE * ICON_SIZE * 4) as usize,
            "icon-64.rgba 必须是 64x64 的 RGBA 数据"
        );
    }
}
