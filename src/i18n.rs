//! 极简国际化：界面文案以中文为 key，英文为默认展示语言。
//!
//! 用法：
//! * `t("连接设置")` —— 取单条文案（`&'static str`）；
//! * `tt!("已加载 {}", name)` —— 取模板并填充 `{}` 占位符，返回 `String`。
//!
//! 语言保存在用户配置目录下的 `language` 文件中（内容为 `en` / `zh`）。

use std::sync::atomic::{AtomicU8, Ordering};

use crate::custom;

/// 界面语言。默认英文。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Lang {
    #[default]
    En,
    Zh,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Zh => "zh",
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        match code.trim().to_ascii_lowercase().as_str() {
            "en" | "en-us" | "en_us" => Some(Self::En),
            "zh" | "zh-cn" | "zh_cn" | "cn" => Some(Self::Zh),
            _ => None,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(Lang::En as u8);

pub fn lang() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        0 => Lang::En,
        _ => Lang::Zh,
    }
}

/// 切换语言（仅影响内存中的全局状态）。
pub fn set_lang(lang: Lang) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

fn language_file() -> std::path::PathBuf {
    custom::config_dir().join("language")
}

/// 从配置目录读取上次选择的语言（不存在或非法时返回英文）。
pub fn load_persisted() -> Lang {
    std::fs::read_to_string(language_file())
        .ok()
        .and_then(|text| Lang::from_code(&text))
        .unwrap_or_default()
}

/// 保存语言选择（失败时静默忽略，不影响使用）。
pub fn persist(lang: Lang) {
    let path = language_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, lang.code());
}

/// 取单条文案；英文模式下在对照表中查找，找不到时回退为原文。
pub fn t(zh: &'static str) -> &'static str {
    if lang() == Lang::Zh { zh } else { en_for(zh) }
}

/// 按顺序填充模板中的 `{}` 占位符。
pub fn fill(template: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(template.len() + 32);
    let mut parts = template.split("{}");
    if let Some(first) = parts.next() {
        out.push_str(first);
    }
    for (index, part) in parts.enumerate() {
        if let Some(arg) = args.get(index) {
            out.push_str(arg);
        }
        out.push_str(part);
    }
    out
}

/// 厂家名本地化：中文模式保留「English(中文)」，英文模式只保留英文部分。
pub fn vendor(name: &str) -> String {
    if lang() == Lang::Zh {
        return name.to_string();
    }
    match name {
        "自定义芯片(导入)" => return "Custom (imported)".to_string(),
        "其他厂家" => return "Other vendors".to_string(),
        _ => {}
    }
    match name.split_once('(') {
        Some((english, _)) => english.trim().to_string(),
        None => name.to_string(),
    }
}

fn en_for(zh: &'static str) -> &'static str {
    match zh {
        // ---- 通用 / 状态 ----
        "就绪" => "Ready",
        "正在初始化，请稍候…" => "Initializing, please wait…",
        "设置" => "Settings",
        "语言" => "Language",
        "关闭" => "Close",
        "取消" => "Cancel",
        "删除" => "Delete",
        "确认删除" => "Confirm",
        "清除" => "Clear",
        "导出" => "Export",
        "打开" => "Open",
        "保存" => "Save",
        "导入" => "Import",
        "转到" => "Go",
        "路径:" => "Path:",
        "文件名:" => "File name:",
        "路径不存在" => "Path does not exist",
        "[目录] {}" => "[Dir] {}",
        "应用信息" => "About",

        // ---- 标题栏 / 面板 ----
        "连接设置" => "Connection",
        "芯片选择" => "Chip",
        "烧录操作" => "Flash operations",
        "固件" => "Firmware",
        "日志" => "Log",

        // ---- 连接 ----
        "设备:" => "Probe:",
        "端口:" => "Interface:",
        "速率:" => "Speed:",
        "复位:" => "Reset:",
        "未检测到调试器" => "No debug probe detected",
        "选择调试器" => "Select a probe",

        // ---- 复位方式 ----
        "默认" => "Default",
        "连接时复位" => "Under reset",
        "硬件复位" => "Hard reset",

        // ---- 芯片 ----
        "芯片库加载中…" => "Loading chip database…",
        "厂家:" => "Vendor:",
        "系列:" => "Series:",
        "型号:" => "Chip:",
        "无匹配型号" => "No matching chip",
        "输入型号关键字，如 STM32" => "Search chip model, e.g. STM32",
        "导入芯片…" => "Import chip…",
        "管理自定义" => "Manage custom",
        "芯片型号已更改，重新连接后生效" => "Chip changed; reconnect to apply",

        // ---- 操作按钮 ----
        "连接" => "Connect",
        "烧录" => "Flash",
        "校验" => "Verify",
        "擦除" => "Erase",
        "读取" => "Read",
        "复位" => "Reset",
        "读写内存" => "Memory",
        "终止" => "Abort",
        "读内存" => "Read memory",
        "写内存" => "Write memory",
        "导入芯片" => "Import chip",
        "删除芯片" => "Remove chip",
        "填充" => "Fill",
        "已请求终止当前操作" => "Abort requested",
        "操作已终止" => "Operation aborted",
        "整片擦除" => "Chip erase",
        "自动烧录" => "Auto flash",
        "自动连接" => "Auto connect",

        // ---- 固件拖拽区 ----
        "拖拽 .bin / .hex 文件到此处" => "Drop a .bin / .hex file here",
        "支持固件文件快速加载" => "Quick firmware loading",
        "点击此处选择文件" => "Click to choose a file",
        "目标" => "Target",
        "{}: {} · {} · 起始 0x{} · CRC32 {}" => "{}: {} · {} · start 0x{} · CRC32 {}",

        // ---- 状态栏 ----
        "目标 {} 已连接" => "Target {} connected",
        "正在连接目标…" => "Connecting to target…",
        "未连接目标" => "No target connected",

        // ---- 进度 ----
        "{}中…" => "{}…",
        "开始{}…" => "Starting {}…",
        "{}完成 {} ms" => "{} finished {} ms",
        "{}失败" => "{} failed",
        "{}已终止" => "{} aborted",

        // ---- 日志消息 ----
        "已切换到自定义芯片: {}" => "Switched to custom chip: {}",
        "默认芯片型号: {}" => "Default chip: {}",
        "bin 固件基地址: 0x{} (Flash {})" => "BIN base address: 0x{} (Flash {})",
        "已读取 {} 字节" => "Read {} bytes",
        "自动烧录已触发" => "Auto-flash triggered",
        "请先加载固件文件" => "Load a firmware file first",
        "未连接目标芯片，无法烧录" => "No target connected, cannot flash",
        "***************** 烧录 {} *****************" => {
            "***************** Flash {} *****************"
        }
        "***************** 校验 *****************" => {
            "***************** Verify *****************"
        }
        "***************** 擦除 *****************" => "***************** Erase *****************",
        "***************** 读取 *****************" => "***************** Read *****************",
        "正在打开 {} …" => "Opening {} …",
        "已打开 {}: {}" => "Opened {}: {}",
        "合计 {}, 起始 0x{}, 视图显示 {}, 耗时 {} ms" => {
            "Total {}, start 0x{}, view {}, took {} ms"
        }
        "已加载 {}" => "Loaded {}",
        "Flash 布局就绪，共 {} 个扇区" => "Flash layout ready, {} sectors",
        "Probe-rs Flasher V0.1 已启动" => "Probe-rs Flasher V0.1 started",
        "检测到 {} 个调试器: {}" => "Detected {} probe(s): {}",
        "跳过自定义芯片 {}: {}" => "Skipping custom chip {}: {}",
        "，含 {} 个自定义系列" => ", including {} custom series",
        "芯片库加载完成: {} 个系列 / {} 个型号{} ({} ms)" => {
            "Chip database loaded: {} series / {} chips{} ({} ms)"
        }
        "正在解析自定义芯片文件 {} …" => "Parsing custom chip file {} …",
        "已保存自定义芯片: {} ({} 个型号) -> {}" => {
            "Saved custom chip: {} ({} chips) -> {}"
        }
        "导入完成 {} ms, {} 个系列 / {} 个型号" => {
            "Import finished {} ms, {} series / {} chips"
        }
        "未找到自定义芯片系列: {}" => "Custom chip series not found: {}",
        "删除 {} 失败: {}" => "Failed to delete {}: {}",
        "已删除自定义芯片: {} ({} 个型号)" => "Removed custom chip: {} ({} chips)",

        // ---- 连接 / 烧录 ----
        "打开调试器 {} 失败: {}" => "Failed to open probe {}: {}",
        "未检测到调试器，请检查 USB 连接与驱动" => {
            "No debug probe detected; check the USB connection and drivers"
        }
        "切换到 {} 失败: {}" => "Failed to switch to {}: {}",
        "调试器不支持 {} MHz，已使用 {} kHz" => {
            "Probe does not support {} MHz; using {} kHz"
        }
        "设置速率失败（继续使用默认速率）: {}" => {
            "Failed to set speed (keeping default): {}"
        }
        "连接目标芯片 {} 失败: {}" => "Failed to connect to target {}: {}",
        "已连接目标 {} · {}" => "Connected to target {} · {}",
        "未连接目标，请先连接调试器" => "No target connected; connect a probe first",
        "解析固件失败: {}" => "Failed to parse firmware: {}",
        "烧录失败: {}" => "Flash failed: {}",
        "烧录完成 {} ms, 共 {}" => "Flashing finished {} ms, {} total",
        "校验失败: {}" => "Verify failed: {}",
        "校验完成 {} ms" => "Verification finished {} ms",
        "擦除失败: {}" => "Erase failed: {}",
        "擦除完成 {} ms" => "Erase finished {} ms",
        "无法访问内核: {}" => "Cannot access core: {}",
        "复位失败: {}" => "Reset failed: {}",
        "复位完成 {} ms" => "Reset finished {} ms",

        // ---- 读取 / 内存 ----
        "目标芯片没有可读取的 Flash 区域" => "Target has no readable flash region",
        "读取 {}%" => "Read {}%",
        "没有读取到数据" => "No data read",
        "目标内存" => "Target memory",
        "内存" => "Memory",
        "读取完成 {} ms, 起始 0x{}, 共 {}" => "Read finished {} ms, from 0x{}, {} total",
        "读取内存完成 {} ms, 0x{} 起 {} 字节" => {
            "Memory read finished {} ms, {} bytes from 0x{}"
        }
        "写入内存失败 @ 0x{}: {}" => "Memory write failed @ 0x{}: {}",
        "写入内存完成 {} ms, 0x{} 起 {} 字节" => {
            "Memory write finished {} ms, {} bytes from 0x{}"
        }
        "读取内存失败 @ 0x{}: {}" => "Memory read failed @ 0x{}: {}",
        "正在读取…" => "Reading…",
        "正在写入 {} 字节…" => "Writing {} bytes…",
        "长度必须为正整数" => "Length must be a positive integer",
        "读取结果将显示在这里" => "Read results will appear here",
        "地址" => "Address",
        "长度" => "Length",
        "写入" => "Write",
        "写入数据（十六进制，可用空格分隔）" => {
            "Write data (hex, space separated)"
        }
        "例如: 01 02 03 04 …" => "e.g. 01 02 03 04 …",
        "没有可写入的数据" => "No data to write",

        // ---- 自定义芯片窗口 ----
        "自定义芯片" => "Custom chips",
        "存放目录:" => "Directory:",
        "打开目录" => "Open folder",
        "还没有导入自定义芯片，点击下方「导入芯片…」开始。" => {
            "No custom chips imported yet. Use “Import chip…” below to start."
        }
        "{} 个型号" => "{} chips",
        "正在处理，请稍候…" => "Working, please wait…",
        "提示：导入 .pdsc 时会扫描其所在目录以获取 flash 算法文件" => {
            "Tip: importing a .pdsc scans its directory for flash algorithm files"
        }
        "支持 CMSIS-Pack（.pack / 解压目录 / .pdsc）与 probe-rs 目标描述 YAML，由 target-gen 生成并保存到本地配置目录。" => {
            "Supports CMSIS-Packs (.pack / extracted folder / .pdsc) and probe-rs target description YAML, generated by target-gen and stored in the local config directory."
        }
        "自定义芯片(导入)" => "Custom (imported)",
        "其他厂家" => "Other vendors",
        "{} (其他)" => "{} (other)",

        // ---- 文件选择器 ----
        "打开固件文件" => "Open firmware file",
        "导出为文件" => "Export to file",
        "导入自定义芯片" => "Import custom chip",
        "已导出 {} ({})" => "Exported {} ({})",
        "导出完成" => "Export complete",
        "写入文件失败: {}" => "Failed to write file: {}",
        "无法读取目录: {}" => "Cannot read directory: {}",
        "无法打开目录 {}: {}" => "Cannot open directory {}: {}",
        "已清除固件视图" => "Firmware view cleared",

        // ---- 固件解析 ----
        "不支持的文件类型: {}" => "Unsupported file type: {}",
        "读取文件失败: {}" => "Failed to read file: {}",
        "文件中没有可用的数据" => "No usable data in file",
        "镜像过大（{}），当前仅支持 {} 以内的镜像" => {
            "Image too large ({}); only images up to {} are supported"
        }
        "文件中没有数据段" => "No data segments in file",
        "镜像地址跨度 {} 过大" => "Image address span {} is too large",
        "HEX 解析失败: {}" => "HEX parsing failed: {}",
        "ELF 解析失败: {}" => "ELF parsing failed: {}",
        "ELF 段数据读取失败: {}" => "Failed to read ELF segment: {}",
        "ELF 中没有可加载的段" => "No loadable segments in ELF",
        "UF2 文件长度不是 512 的整数倍" => "UF2 length is not a multiple of 512",
        "UF2 第 {} 块魔数不匹配" => "UF2 block {} has a bad magic number",
        "请输入要写入的数据" => "Enter data to write",
        "十六进制字符个数必须为偶数" => "Hex character count must be even",
        "包含非法字符" => "Contains invalid characters",
        "请输入地址" => "Enter an address",
        "无法解析地址: {}" => "Cannot parse address: {}",

        // ---- 自定义芯片解析 ----
        "无法创建目录 {}: {}" => "Cannot create directory {}: {}",
        "读取 {} 失败: {}" => "Failed to read {}: {}",
        "目标描述 YAML 解析失败: {}" => "Failed to parse target description YAML: {}",
        "序列化芯片系列失败: {}" => "Failed to serialize chip series: {}",
        "路径不存在: {}" => "Path does not exist: {}",
        "解析 CMSIS-Pack 目录失败: {}" => "Failed to parse CMSIS-Pack directory: {}",
        "解析 CMSIS-Pack 失败: {}" => "Failed to parse CMSIS-Pack: {}",
        "无法确定 .pdsc 文件所在目录" => {
            "Cannot determine the directory containing the .pdsc file"
        }
        "不支持的文件类型: {}（支持 .pack / .pdsc / .yaml，或 Pack 解压目录）" => {
            "Unsupported file type: {} (expected .pack / .pdsc / .yaml, or an extracted pack directory)"
        }
        "没有生成任何可用的芯片系列（该 Pack 可能缺少 flash 算法或使用了不支持的芯片内核）" => {
            "No usable chip series were generated (the pack may lack flash algorithms or use an unsupported core)"
        }
        "写入 {} 失败: {}" => "Failed to write {}: {}",
        "没有可保存的芯片系列" => "No chip series to save",

        _ => zh,
    }
}

/// 取模板并填充 `{}` 占位符。参数会按 `Display` 转成字符串。
#[macro_export]
macro_rules! tt {
    ($tpl:literal) => {
        $crate::i18n::t($tpl).to_string()
    };
    ($tpl:literal, $($arg:expr),+ $(,)?) => {
        $crate::i18n::fill(
            $crate::i18n::t($tpl),
            &[$($arg.to_string()),+],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_defaults_and_codes() {
        assert_eq!(Lang::default(), Lang::En);
        assert_eq!(Lang::from_code("en"), Some(Lang::En));
        assert_eq!(Lang::from_code("EN-us"), Some(Lang::En));
        assert_eq!(Lang::from_code(" zh-cn "), Some(Lang::Zh));
        assert_eq!(Lang::from_code("klingon"), None);
    }

    #[test]
    fn translation_lookup_and_fill() {
        // 全局语言状态，放在同一个测试里避免并行测试互相干扰。
        set_lang(Lang::En);
        assert_eq!(t("连接设置"), "Connection");
        assert_eq!(t("未收录的文案"), "未收录的文案");
        assert_eq!(
            fill(t("已加载 {}"), &["demo.bin".to_string()]),
            "Loaded demo.bin"
        );
        set_lang(Lang::Zh);
        assert_eq!(t("连接设置"), "连接设置");
        assert_eq!(
            vendor("STMicroelectronics(意法)"),
            "STMicroelectronics(意法)"
        );
        set_lang(Lang::En);
        assert_eq!(vendor("STMicroelectronics(意法)"), "STMicroelectronics");
        assert_eq!(vendor("自定义芯片(导入)"), "Custom (imported)");
    }
}
