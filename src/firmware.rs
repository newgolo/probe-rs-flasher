//! 固件文件解析：.bin / .hex / .elf / .uf2 -> 连续地址空间，用于十六进制视图与烧录。

use std::path::{Path, PathBuf};

use object::{Object, ObjectSegment};

use crate::tt;

/// 十六进制视图允许的最大体积，避免恶意/超大文件导致内存爆炸。
pub const MAX_VIEW_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FirmwareFormat {
    Bin,
    Hex,
    Elf,
    Uf2,
}

impl FirmwareFormat {
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())?;
        match ext.as_str() {
            "bin" | "binary" => Some(Self::Bin),
            "hex" | "ihex" | "intelhex" => Some(Self::Hex),
            "elf" | "axf" | "out" => Some(Self::Elf),
            "uf2" => Some(Self::Uf2),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Bin => "bin",
            Self::Hex => "hex",
            Self::Elf => "elf",
            Self::Uf2 => "uf2",
        }
    }
}

/// 已加载的固件镜像。
#[derive(Clone, Debug)]
pub struct LoadedFirmware {
    pub path: PathBuf,
    pub name: String,
    pub format: FirmwareFormat,
    /// 视图中第一个字节所在的地址。
    pub base: u64,
    /// 烧录时使用的基地址（bin 文件需要；hex/elf/uf2 自带地址）。
    pub load_base: u64,
    pub bytes: Vec<u8>,
    pub crc32: u32,
}

impl LoadedFirmware {
    pub fn size_text(&self) -> String {
        format_bytes(self.bytes.len() as u64)
    }
}

pub fn load_firmware(path: &Path, fallback_base: u64) -> Result<LoadedFirmware, String> {
    let format = FirmwareFormat::from_path(path).ok_or_else(|| {
        tt!(
            "不支持的文件类型: {}",
            path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string())
        )
    })?;

    let raw = std::fs::read(path).map_err(|e| tt!("读取文件失败: {}", e))?;

    let (base, bytes) = match format {
        FirmwareFormat::Bin => (fallback_base, raw),
        FirmwareFormat::Hex => parse_hex(&raw)?,
        FirmwareFormat::Elf => parse_elf(&raw)?,
        FirmwareFormat::Uf2 => parse_uf2(&raw)?,
    };

    if bytes.is_empty() {
        return Err(tt!("文件中没有可用的数据"));
    }
    if bytes.len() > MAX_VIEW_BYTES {
        return Err(tt!(
            "镜像过大（{}），当前仅支持 {} 以内的镜像",
            format_bytes(bytes.len() as u64),
            format_bytes(MAX_VIEW_BYTES as u64)
        ));
    }

    let crc32 = crc32fast::hash(&bytes);
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    Ok(LoadedFirmware {
        path: path.to_path_buf(),
        name,
        format,
        base,
        load_base: base,
        bytes,
        crc32,
    })
}

fn merge_chunks(mut chunks: Vec<(u64, Vec<u8>)>) -> Result<(u64, Vec<u8>), String> {
    chunks.retain(|(_, d)| !d.is_empty());
    if chunks.is_empty() {
        return Err(tt!("文件中没有数据段"));
    }
    chunks.sort_by_key(|(addr, _)| *addr);

    let start = chunks[0].0;
    let end = chunks
        .iter()
        .map(|(addr, data)| addr.saturating_add(data.len() as u64))
        .max()
        .unwrap_or(start);
    let len = end.saturating_sub(start) as usize;
    if len > MAX_VIEW_BYTES {
        return Err(tt!("镜像地址跨度 {} 过大", format_bytes(len as u64)));
    }

    let mut buffer = vec![0xFFu8; len];
    for (addr, data) in chunks {
        let offset = (addr - start) as usize;
        let end_off = (offset + data.len()).min(len);
        buffer[offset..end_off].copy_from_slice(&data[..end_off - offset]);
    }
    Ok((start, buffer))
}

fn parse_hex(raw: &[u8]) -> Result<(u64, Vec<u8>), String> {
    let text = String::from_utf8_lossy(raw);
    let mut chunks: Vec<(u64, Vec<u8>)> = Vec::new();
    let mut base: u64 = 0;

    for record in ihex::Reader::new(&text) {
        match record.map_err(|e| tt!("HEX 解析失败: {}", e))? {
            ihex::Record::Data { offset, value } => {
                chunks.push((base + offset as u64, value));
            }
            ihex::Record::ExtendedSegmentAddress(seg) => {
                base = (seg as u64) << 4;
            }
            ihex::Record::ExtendedLinearAddress(upper) => {
                base = (upper as u64) << 16;
            }
            ihex::Record::StartSegmentAddress { .. }
            | ihex::Record::StartLinearAddress(_)
            | ihex::Record::EndOfFile => {}
        }
    }

    merge_chunks(chunks)
}

fn parse_elf(raw: &[u8]) -> Result<(u64, Vec<u8>), String> {
    let file = object::File::parse(raw).map_err(|e| tt!("ELF 解析失败: {}", e))?;

    let mut chunks: Vec<(u64, Vec<u8>)> = Vec::new();
    for segment in file.segments() {
        let address = segment.address();
        let data = segment
            .data()
            .map_err(|e| tt!("ELF 段数据读取失败: {}", e))?;
        // 跳过地址为 0 的段（通常是文件头/未映射区域）
        if address == 0 || data.is_empty() {
            continue;
        }
        chunks.push((address, data.to_vec()));
    }

    if chunks.is_empty() {
        return Err(tt!("ELF 中没有可加载的段"));
    }
    merge_chunks(chunks)
}

fn parse_uf2(raw: &[u8]) -> Result<(u64, Vec<u8>), String> {
    const MAGIC_START0: u32 = 0x0A32_4655;
    const MAGIC_START1: u32 = 0x9E5D_5157;
    const MAGIC_END: u32 = 0x0AB1_6F30;

    if !raw.len().is_multiple_of(512) {
        return Err(tt!("UF2 文件长度不是 512 的整数倍"));
    }

    let mut chunks: Vec<(u64, Vec<u8>)> = Vec::new();
    for (index, block) in raw.chunks_exact(512).enumerate() {
        let read_u32 = |at: usize| -> u32 {
            u32::from_le_bytes([block[at], block[at + 1], block[at + 2], block[at + 3]])
        };
        if read_u32(0) != MAGIC_START0 || read_u32(4) != MAGIC_START1 || read_u32(508) != MAGIC_END
        {
            return Err(tt!("UF2 第 {} 块魔数不匹配", index));
        }
        let address = read_u32(12) as u64;
        let payload = &block[32..32 + 256];
        chunks.push((address, payload.to_vec()));
    }

    merge_chunks(chunks)
}

pub fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * 1024.0;

    let n_f = n as f64;
    if n_f >= MB {
        let v = n_f / MB;
        if (v - v.round()).abs() < 0.05 {
            format!("{v:.0} MB")
        } else {
            format!("{v:.1} MB")
        }
    } else if n_f >= KB {
        let v = n_f / KB;
        if (v - v.round()).abs() < 0.05 {
            format!("{v:.0} KB")
        } else {
            format!("{v:.1} KB")
        }
    } else {
        format!("{n} B")
    }
}

/// 生成一段内存区域的十六进制文本（用于日志/内存窗口）。
pub fn hexdump_text(base: u64, bytes: &[u8], max_bytes: usize) -> String {
    let bytes = &bytes[..bytes.len().min(max_bytes)];
    let mut out = String::new();
    for (row, chunk) in bytes.chunks(16).enumerate() {
        out.push_str(&format!("{:08X}  ", base + (row * 16) as u64));
        for i in 0..16 {
            match chunk.get(i) {
                Some(b) => out.push_str(&format!("{b:02X} ")),
                None => out.push_str("   "),
            }
        }
        out.push(' ');
        for b in chunk {
            out.push(if (0x20..0x7F).contains(b) {
                *b as char
            } else {
                '.'
            });
        }
        out.push('\n');
    }
    out
}

/// 解析用户输入的十六进制字节串，例如 "00 11 aBcD"、"0x00,0x11"。
pub fn parse_hex_bytes(text: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = text
        .replace("0x", " ")
        .replace("0X", " ")
        .chars()
        .filter(|c| {
            c.is_ascii_hexdigit() || *c == ' ' || *c == ',' || *c == ';' || *c == '\n' || *c == '\t'
        })
        .collect();

    let digits: String = cleaned
        .split(|c: char| c.is_whitespace() || c == ',' || c == ';')
        .collect::<Vec<_>>()
        .join("");

    if digits.is_empty() {
        return Err(tt!("请输入要写入的数据"));
    }
    if !digits.len().is_multiple_of(2) {
        return Err(tt!("十六进制字符个数必须为偶数"));
    }

    let mut out = Vec::with_capacity(digits.len() / 2);
    let raw = digits.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        let hi = (raw[i] as char)
            .to_digit(16)
            .ok_or_else(|| tt!("包含非法字符"))?;
        let lo = (raw[i + 1] as char)
            .to_digit(16)
            .ok_or_else(|| tt!("包含非法字符"))?;
        out.push((hi * 16 + lo) as u8);
        i += 2;
    }
    Ok(out)
}

/// 解析地址输入，支持 "0x08000000" / "08000000" / "134217728"。
///
/// 规则：带 `0x` 前缀或包含 a-f 的一律按十六进制解析；纯数字按十进制解析，
/// 但带前导 0 的纯数字（如 `08000000`）按十六进制解析，符合地址书写的习惯。
pub fn parse_address(text: &str) -> Result<u64, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err(tt!("请输入地址"));
    }

    let is_plain_digits = t.chars().all(|c| c.is_ascii_digit());
    let looks_hex = !is_plain_digits || (t.len() > 1 && t.starts_with('0'));

    let parsed = if let Some(rest) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(rest, 16)
    } else if looks_hex {
        u64::from_str_radix(t, 16)
    } else {
        t.parse::<u64>()
    };
    parsed.map_err(|_| tt!("无法解析地址: {}", t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_bytes_roundtrip() {
        let bytes = parse_hex_bytes("0xDE 0xAD, be ef").unwrap();
        assert_eq!(bytes, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn hexdump_is_aligned() {
        let text = hexdump_text(0x0800_0000, &[0x11, 0x22, 0x41], 1024);
        assert!(text.starts_with("08000000  11 22 41 "));
        // 0x22 是可见字符 '"'，0x11 不可见显示为 '.'
        assert!(text.trim_end().ends_with(".\"A"));
    }

    #[test]
    fn addresses() {
        assert_eq!(parse_address("0x08000000").unwrap(), 0x0800_0000);
        assert_eq!(parse_address("08000000").unwrap(), 0x0800_0000);
        assert_eq!(parse_address("1024").unwrap(), 1024);
    }

    fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
        let dir = std::env::temp_dir().join("probe-rs-flasher-tests");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn load_hex_file() {
        let path = temp_file(
            "sample.hex",
            b":020000040800F2\n:0400000001020304F2\n:00000001FF\n",
        );
        let fw = load_firmware(&path, 0).unwrap();
        assert_eq!(fw.format, FirmwareFormat::Hex);
        assert_eq!(fw.base, 0x0800_0000);
        assert_eq!(fw.bytes, vec![1, 2, 3, 4]);
        assert_eq!(fw.crc32, crc32fast::hash(&[1, 2, 3, 4]));
    }

    #[test]
    fn load_uf2_file() {
        let mut block = vec![0u8; 512];
        block[0..4].copy_from_slice(&0x0A32_4655u32.to_le_bytes());
        block[4..8].copy_from_slice(&0x9E5D_5157u32.to_le_bytes());
        block[12..16].copy_from_slice(&0x1000u32.to_le_bytes());
        block[16..20].copy_from_slice(&256u32.to_le_bytes());
        block[24..28].copy_from_slice(&1u32.to_le_bytes());
        block[32..36].copy_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);
        block[508..512].copy_from_slice(&0x0AB1_6F30u32.to_le_bytes());

        let path = temp_file("sample.uf2", &block);
        let fw = load_firmware(&path, 0).unwrap();
        assert_eq!(fw.format, FirmwareFormat::Uf2);
        assert_eq!(fw.base, 0x1000);
        assert_eq!(fw.bytes.len(), 256);
        assert_eq!(&fw.bytes[..4], &[0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn rejects_unknown_extension() {
        let path = temp_file("sample.txt", b"hello");
        assert!(load_firmware(&path, 0).is_err());
    }

    #[test]
    fn sizes() {
        assert_eq!(format_bytes(256 * 1024), "256 KB");
        assert_eq!(format_bytes(1024 * 1024), "1 MB");
        assert_eq!(format_bytes(512), "512 B");
    }
}
