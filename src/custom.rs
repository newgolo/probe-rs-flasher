//! 自定义芯片支持：导入 CMSIS-Pack（`.pack` / 解压目录 / `.pdsc`）或目标描述 YAML，
//! 使用 `target-gen` 生成 probe-rs 的芯片系列描述并持久化到用户配置目录。

use std::path::{Path, PathBuf};

use probe_rs::config::ChipFamily;

use crate::tt;

/// 自定义芯片在「厂家」下拉框中的分组名。
pub const CUSTOM_VENDOR: &str = "自定义芯片(导入)";

/// 可通过该环境变量覆盖自定义芯片的存放目录（便于测试或便携使用）。
const TARGET_DIR_ENV: &str = "PROBE_RS_FLASHER_TARGETS";

/// 一条已导入的自定义芯片系列记录（也用于 UI 列表展示）。
#[derive(Clone, Debug)]
pub struct CustomTargetInfo {
    pub name: String,
    pub path: PathBuf,
    pub chips: usize,
}

/// 应用配置目录（语言、自定义芯片等）。
pub fn config_dir() -> PathBuf {
    let base = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|home| home.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
    }
    .unwrap_or_else(|| PathBuf::from("."));

    base.join("probe-rs-flasher")
}

/// 自定义芯片描述文件存放目录。
pub fn targets_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os(TARGET_DIR_ENV) {
        return PathBuf::from(dir);
    }
    config_dir().join("targets")
}

pub fn ensure_targets_dir() -> Result<PathBuf, String> {
    let dir = targets_dir();
    std::fs::create_dir_all(&dir).map_err(|e| tt!("无法创建目录 {}: {}", dir.display(), e))?;
    Ok(dir)
}

/// 文件名只保留常见安全字符。
pub fn sanitize_file_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
            out.push(c);
        } else if c.is_whitespace() || c == '/' || c == '\\' || c == ':' {
            out.push('_');
        }
    }
    let out = out.trim_matches('_').to_string();
    if out.is_empty() {
        "custom_family".to_string()
    } else {
        out
    }
}

/// 读取已保存的自定义芯片 YAML。返回 (文件路径, 内容)，解析失败由调用方处理。
pub fn load_saved_yamls() -> Vec<(PathBuf, String)> {
    let dir = targets_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let is_yaml = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml"))
            .unwrap_or(false);
        if !is_yaml {
            continue;
        }
        match std::fs::read_to_string(&path) {
            Ok(text) => files.push((path, text)),
            Err(e) => log_warn(&tt!("读取 {} 失败: {}", path.display(), e)),
        }
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

pub fn parse_family_yaml(yaml: &str) -> Result<ChipFamily, String> {
    yaml_serde::from_str::<ChipFamily>(yaml).map_err(|e| tt!("目标描述 YAML 解析失败: {}", e))
}

/// 与 probe-rs 内置目标文件保持一致的 YAML 序列化。
pub fn serialize_family(family: &ChipFamily) -> Result<String, String> {
    target_gen::commands::elf::serialize_to_yaml_string(family)
        .map_err(|e| tt!("序列化芯片系列失败: {}", format!("{e:#}")))
}

/// 从用户选择的文件 / 目录生成芯片系列。
///
/// 支持：
/// - `.pack` / `.zip`：CMSIS-Pack 压缩包
/// - 目录：包含 `.pdsc` 与算法文件的解压包目录
/// - `.pdsc`：只支持其所在目录中完整的 Pack 内容（会扫描该目录）
/// - `.yaml` / `.yml`：target-gen / probe-rs 目标描述
pub fn generate_from_path(path: &Path) -> Result<Vec<ChipFamily>, String> {
    if !path.exists() {
        return Err(tt!("路径不存在: {}", path.display()));
    }

    let mut families: Vec<ChipFamily> = Vec::new();

    if path.is_dir() {
        target_gen::generate::visit_dirs(path, &mut families)
            .map_err(|e| tt!("解析 CMSIS-Pack 目录失败: {}", format!("{e:#}")))?;
    } else {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();

        match ext.as_str() {
            "pack" | "zip" => {
                target_gen::generate::visit_file(path, &mut families)
                    .map_err(|e| tt!("解析 CMSIS-Pack 失败: {}", format!("{e:#}")))?;
            }
            "pdsc" => {
                let dir = path
                    .parent()
                    .ok_or_else(|| tt!("无法确定 .pdsc 文件所在目录"))?;
                target_gen::generate::visit_dirs(dir, &mut families)
                    .map_err(|e| tt!("解析 CMSIS-Pack 目录失败: {}", format!("{e:#}")))?;
            }
            "yaml" | "yml" => {
                let text = std::fs::read_to_string(path)
                    .map_err(|e| tt!("读取 {} 失败: {}", path.display(), e))?;
                families.push(parse_family_yaml(&text)?);
            }
            _ => {
                return Err(tt!(
                    "不支持的文件类型: {}（支持 .pack / .pdsc / .yaml，或 Pack 解压目录）",
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.display().to_string())
                ));
            }
        }
    }

    // 去掉没有任何型号的系列（例如 Pack 中不支持的核）
    families.retain(|family| !family.variants.is_empty());

    if families.is_empty() {
        return Err(tt!(
            "没有生成任何可用的芯片系列（该 Pack 可能缺少 flash 算法或使用了不支持的芯片内核）"
        ));
    }

    Ok(families)
}

/// 保存芯片系列到自定义芯片目录，返回保存结果。
pub fn save_families(families: &[ChipFamily]) -> Result<Vec<CustomTargetInfo>, String> {
    let dir = ensure_targets_dir()?;
    let mut saved = Vec::new();

    for family in families {
        if family.variants.is_empty() {
            continue;
        }
        let yaml = serialize_family(family)?;
        let file_name = format!("{}.yaml", sanitize_file_name(&family.name));
        let path = dir.join(&file_name);
        std::fs::write(&path, yaml).map_err(|e| tt!("写入 {} 失败: {}", path.display(), e))?;
        saved.push(CustomTargetInfo {
            name: family.name.clone(),
            path,
            chips: family.variants.len(),
        });
    }

    if saved.is_empty() {
        return Err(tt!("没有可保存的芯片系列"));
    }
    Ok(saved)
}

fn log_warn(message: &str) {
    eprintln!("{message}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_sanitizing() {
        assert_eq!(sanitize_file_name("STM32F1 Series"), "STM32F1_Series");
        assert_eq!(sanitize_file_name("ACME/Chip:1"), "ACME_Chip_1");
        assert_eq!(sanitize_file_name("   "), "custom_family");
    }

    #[test]
    fn targets_dir_can_be_overridden() {
        // 仅验证路径拼接逻辑，不做真实文件写操作
        let dir = targets_dir();
        assert!(dir.to_string_lossy().contains("targets"));
    }

    /// 使用 target-gen 仓库自带的示例 Pack 做端到端测试（不存在时跳过）。
    #[test]
    fn import_pack_with_target_gen() {
        let pack = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../target-gen/tests/test_data/NordicSemiconductor.nRF_DeviceFamilyPack.8.32.1.pack",
        );
        if !pack.exists() {
            eprintln!("跳过测试：未找到示例 Pack {}", pack.display());
            return;
        }

        let families = generate_from_path(&pack).expect("解析 Pack 失败");
        assert!(!families.is_empty());

        let name = families[0].name.clone();
        let chips = families[0].variants.len();
        assert!(chips > 0, "系列 {name} 没有型号");

        let yaml = serialize_family(&families[0]).expect("序列化失败");
        assert!(yaml.contains(&name));

        // 生成的 YAML 必须能被 probe-rs 注册
        let mut registry = probe_rs::config::Registry::from_builtin_families();
        let registered = registry
            .add_target_family_from_yaml(&yaml)
            .expect("probe-rs 无法注册生成的芯片系列");
        assert_eq!(registered, name);
    }
}
