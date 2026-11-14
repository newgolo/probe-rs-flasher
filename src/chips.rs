//! 芯片库：从 probe-rs 内置的目标描述中整理出「厂家 / 系列 / 型号」三级列表。

use probe_rs::config::Registry;
use std::collections::HashMap;

use crate::custom::CUSTOM_VENDOR;

#[derive(Clone, Debug, Default)]
pub struct ChipDb {
    pub vendors: Vec<Vendor>,
}

#[derive(Clone, Debug)]
pub struct Vendor {
    pub name: String,
    rank: usize,
    pub families: Vec<Family>,
}

#[derive(Clone, Debug)]
pub struct Family {
    pub name: String,
    pub chips: Vec<String>,
}

impl ChipDb {
    #[cfg(test)]
    pub fn build() -> Self {
        Self::from_registry(&Registry::from_builtin_families())
    }

    #[cfg(test)]
    pub fn from_registry(registry: &Registry) -> Self {
        Self::from_registry_with_custom(registry, &[])
    }

    /// 构建芯片库；`custom_families` 中的系列会统一归入「自定义芯片(导入)」分组，并排在最前面。
    pub fn from_registry_with_custom(registry: &Registry, custom_families: &[String]) -> Self {
        let mut vendors: HashMap<String, Vendor> = HashMap::new();

        for family in registry.families() {
            let is_custom = custom_families.iter().any(|name| name == &family.name);
            let (vendor_name, rank) = if is_custom {
                (CUSTOM_VENDOR.to_string(), 0)
            } else {
                vendor_for(&family.name)
            };
            let mut chips: Vec<String> = family.variants.iter().map(|c| c.name.clone()).collect();
            chips.sort();
            chips.dedup();

            if chips.is_empty() {
                continue;
            }

            let vendor = vendors.entry(vendor_name.clone()).or_insert(Vendor {
                name: vendor_name,
                rank,
                families: Vec::new(),
            });
            vendor.families.push(Family {
                name: family.name.clone(),
                chips,
            });
        }

        let mut vendors: Vec<Vendor> = vendors.into_values().collect();
        for vendor in &mut vendors {
            vendor.families.sort_by(|a, b| a.name.cmp(&b.name));
        }
        vendors.sort_by(|a, b| a.rank.cmp(&b.rank).then_with(|| a.name.cmp(&b.name)));

        Self { vendors }
    }

    pub fn total_chips(&self) -> usize {
        self.vendors
            .iter()
            .flat_map(|v| &v.families)
            .map(|f| f.chips.len())
            .sum()
    }

    pub fn total_families(&self) -> usize {
        self.vendors.iter().map(|v| v.families.len()).sum()
    }

    /// 厂家名称（按当前界面语言本地化）。
    pub fn vendor_names(&self) -> Vec<String> {
        self.vendors
            .iter()
            .map(|v| crate::i18n::vendor(&v.name))
            .collect()
    }

    pub fn family_names(&self, vendor: usize) -> Vec<String> {
        self.vendors
            .get(vendor)
            .map(|v| v.families.iter().map(|f| f.name.clone()).collect())
            .unwrap_or_default()
    }

    pub fn chip_names(&self, vendor: usize, family: usize) -> Vec<String> {
        self.vendors
            .get(vendor)
            .and_then(|v| v.families.get(family))
            .map(|f| f.chips.clone())
            .unwrap_or_default()
    }

    pub fn chip_name(&self, vendor: usize, family: usize, chip: usize) -> Option<String> {
        self.vendors
            .get(vendor)?
            .families
            .get(family)?
            .chips
            .get(chip)
            .cloned()
    }

    /// 默认选中 STM32F103VC（与参考界面一致），找不到则退回第一项。
    pub fn default_selection(&self) -> Option<(usize, usize, usize)> {
        if let Some(found) = self.find("STM32F103VC") {
            return Some(found);
        }
        let vendor = 0;
        let family = 0;
        Some((vendor, family, 0))
    }

    pub fn find(&self, chip_name: &str) -> Option<(usize, usize, usize)> {
        for (vi, vendor) in self.vendors.iter().enumerate() {
            for (fi, family) in vendor.families.iter().enumerate() {
                if let Some(ci) = family.chips.iter().position(|c| c == chip_name) {
                    return Some((vi, fi, ci));
                }
            }
        }
        None
    }

    /// 关键字搜索，返回最多 `limit` 条 (厂家, 系列, 型号)。
    pub fn search(&self, query: &str, limit: usize) -> Vec<(usize, usize, usize)> {
        let query = query.trim().to_ascii_uppercase();
        if query.is_empty() {
            return Vec::new();
        }

        let mut result = Vec::new();
        for (vi, vendor) in self.vendors.iter().enumerate() {
            for (fi, family) in vendor.families.iter().enumerate() {
                let family_matches = family.name.to_ascii_uppercase().contains(&query);
                for (ci, chip) in family.chips.iter().enumerate() {
                    if family_matches || chip.to_ascii_uppercase().contains(&query) {
                        result.push((vi, fi, ci));
                        if result.len() >= limit {
                            return result;
                        }
                    }
                }
            }
        }
        result
    }
}

/// 常见厂家（含中文名），顺序即下拉框顺序。
const VENDOR_TABLE: &[(&str, &str)] = &[
    ("STM32", "STMicroelectronics(意法)"),
    ("STM8", "STMicroelectronics(意法)"),
    ("STR7", "STMicroelectronics(意法)"),
    ("SPC5", "STMicroelectronics(意法)"),
    ("GD32", "GigaDevice(兆易创新)"),
    ("AT32", "Artery(雅特力)"),
    ("APM32", "Geehy(极海)"),
    ("HC32", "HDSC(华大半导体)"),
    ("CH32", "WCH(沁恒)"),
    ("CH5", "WCH(沁恒)"),
    ("AIR", "Ai-Thinker(安信可)"),
    ("MM32", "MindMotion(灵动微)"),
    ("ES32", "Eastsoft(东软载波)"),
    ("BAT32", "Beken(博通集成)"),
    ("NRF", "Nordic(北欧半导体)"),
    ("RP2040", "Raspberry Pi(树莓派)"),
    ("RP2350", "Raspberry Pi(树莓派)"),
    ("ESP", "Espressif(乐鑫)"),
    ("MSP", "Texas Instruments(德州仪器)"),
    ("TM4C", "Texas Instruments(德州仪器)"),
    ("CC13", "Texas Instruments(德州仪器)"),
    ("CC23", "Texas Instruments(德州仪器)"),
    ("CC26", "Texas Instruments(德州仪器)"),
    ("CC32", "Texas Instruments(德州仪器)"),
    ("LPC", "NXP(恩智浦)"),
    ("MIMX", "NXP(恩智浦)"),
    ("MK", "NXP(恩智浦)"),
    ("KL", "NXP(恩智浦)"),
    ("SAMD", "Microchip(微芯)"),
    ("SAML", "Microchip(微芯)"),
    ("SAME", "Microchip(微芯)"),
    ("SAM", "Microchip(微芯)"),
    ("ATSAM", "Microchip(微芯)"),
    ("PIC", "Microchip(微芯)"),
    ("AVR", "Microchip(微芯)"),
    ("AT", "Microchip(微芯)"),
    ("R7FA", "Renesas(瑞萨)"),
    ("RA", "Renesas(瑞萨)"),
    ("RX", "Renesas(瑞萨)"),
    ("RL78", "Renesas(瑞萨)"),
    ("EFM32", "Silicon Labs(芯科)"),
    ("EFR32", "Silicon Labs(芯科)"),
    ("XMC", "Infineon(英飞凌)"),
    ("CY8C", "Infineon(英飞凌)"),
    ("PSoC", "Infineon(英飞凌)"),
    ("AURIX", "Infineon(英飞凌)"),
    ("NUC", "Nuvoton(新唐)"),
    ("M0", "Nuvoton(新唐)"),
    ("ADuCM", "Analog Devices(亚德诺)"),
    ("MAX", "Analog Devices(亚德诺)"),
    ("STM32F", "STMicroelectronics(意法)"),
];

fn vendor_for(family: &str) -> (String, usize) {
    let upper = family.to_ascii_uppercase();
    for (rank, (pattern, vendor)) in VENDOR_TABLE.iter().enumerate() {
        if upper.starts_with(&pattern.to_ascii_uppercase()) {
            // 保留 rank 0 给「自定义芯片(导入)」分组
            return ((*vendor).to_string(), rank + 1);
        }
    }

    // 未知厂家：用型号前缀（第一个数字之前的部分）作为分组名。
    let prefix: String = family
        .chars()
        .take_while(|c| !c.is_ascii_digit())
        .collect::<String>()
        .trim()
        .to_string();
    let name = if prefix.is_empty() {
        "其他厂家".to_string()
    } else {
        format!("{prefix}(其他)")
    };
    (name, VENDOR_TABLE.len() + 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vendor_mapping() {
        assert_eq!(vendor_for("STM32F1 Series").0, "STMicroelectronics(意法)");
        assert_eq!(vendor_for("nRF51 Series").0, "Nordic(北欧半导体)");
        assert_eq!(vendor_for("ESP32 Series").0, "Espressif(乐鑫)");
    }

    #[test]
    fn database_contains_stm32f103vc() {
        let db = ChipDb::build();
        assert!(db.total_chips() > 1000);
        let (v, f, c) = db.default_selection().unwrap();
        assert_eq!(db.chip_name(v, f, c).unwrap(), "STM32F103VC");
    }

    #[test]
    fn custom_families_are_grouped_first() {
        let registry = Registry::from_builtin_families();
        let db = ChipDb::from_registry_with_custom(&registry, &["STM32F1 Series".to_string()]);
        assert_eq!(db.vendors[0].name, CUSTOM_VENDOR);
        assert!(
            db.vendors[0]
                .families
                .iter()
                .any(|f| f.name == "STM32F1 Series")
        );

        let builtin = ChipDb::from_registry(&registry);
        assert_eq!(builtin.vendors[0].name, "STMicroelectronics(意法)");
    }

    #[test]
    fn search_works() {
        let db = ChipDb::build();
        let hits = db.search("stm32f103vc", 10);
        assert!(!hits.is_empty());
        let (v, f, c) = hits[0];
        assert_eq!(db.chip_name(v, f, c).unwrap(), "STM32F103VC");
    }
}
