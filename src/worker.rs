//! 后台工作线程：所有 probe-rs 操作都在这里执行，UI 线程只负责显示与收集用户输入。

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use probe_rs::config::Registry;
use probe_rs::flashing::{
    self, BinLoader, BinOptions, DownloadOptions, ElfLoader, ElfOptions, FlashProgress, HexLoader,
    ImageLoader, ProgressEvent, ProgressOperation, Uf2Loader, build_loader,
};
use probe_rs::probe::list::Lister;
use probe_rs::probe::{DebugProbeInfo, DebugProbeSelector, WireProtocol};
use probe_rs::{MemoryInterface, Permissions, Session};

use crate::chips::ChipDb;
use crate::custom::{self, CustomTargetInfo};
use crate::firmware::{FirmwareFormat, format_bytes};
use crate::i18n::t;
use crate::tt;

// ---------------------------------------------------------------------------
// 简单的数据模型（不依赖 probe-rs 类型，方便在 UI 侧使用）
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct ProbeId {
    pub name: String,
    pub serial: Option<String>,
    pub vendor_id: u16,
    pub product_id: u16,
    pub interface: Option<u8>,
}

impl ProbeId {
    pub fn label(&self) -> String {
        match &self.serial {
            Some(serial) if !serial.is_empty() => format!("{} ({})", self.name, serial),
            _ => self.name.clone(),
        }
    }

    fn selector(&self) -> DebugProbeSelector {
        DebugProbeSelector {
            vendor_id: self.vendor_id,
            product_id: self.product_id,
            interface: self.interface,
            serial_number: self.serial.clone(),
        }
    }
}

impl From<&DebugProbeInfo> for ProbeId {
    fn from(info: &DebugProbeInfo) -> Self {
        Self {
            name: info.identifier.clone(),
            serial: info.serial_number.clone(),
            vendor_id: info.vendor_id,
            product_id: info.product_id,
            interface: info.interface,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Protocol {
    Swd,
    Jtag,
}

impl Protocol {
    pub fn label(self) -> &'static str {
        match self {
            Self::Swd => "SWD",
            Self::Jtag => "JTAG",
        }
    }

    fn wire(self) -> WireProtocol {
        match self {
            Self::Swd => WireProtocol::Swd,
            Self::Jtag => WireProtocol::Jtag,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResetMode {
    /// 普通方式连接
    Default,
    /// 复位期间连接（under reset）
    UnderReset,
    /// 连接后硬件复位
    HardReset,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OpKind {
    Connect,
    Flash,
    Verify,
    Erase,
    Reset,
    ReadFlash,
    ReadMemory,
    WriteMemory,
    ImportTarget,
    RemoveTarget,
}

impl OpKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Connect => t("连接"),
            Self::Flash => t("烧录"),
            Self::Verify => t("校验"),
            Self::Erase => t("擦除"),
            Self::Reset => t("复位"),
            Self::ReadFlash => t("读取"),
            Self::ReadMemory => t("读内存"),
            Self::WriteMemory => t("写内存"),
            Self::ImportTarget => t("导入芯片"),
            Self::RemoveTarget => t("删除芯片"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LogLevel {
    Info,
    Ok,
    Warn,
    Error,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DumpKind {
    /// 读取目标 Flash 后的镜像（显示在固件面板）
    Target,
    /// “读写内存” 窗口的读取结果
    MemoryWindow,
}

#[derive(Debug)]
pub enum Command {
    RefreshProbes,
    Attach {
        probe: Option<ProbeId>,
        chip: String,
        protocol: Protocol,
        speed_khz: u32,
        reset: ResetMode,
        quiet: bool,
    },
    Flash {
        path: PathBuf,
        format: FirmwareFormat,
        base_address: u64,
        chip_erase: bool,
        verify: bool,
    },
    Verify {
        path: PathBuf,
        format: FirmwareFormat,
        base_address: u64,
    },
    Erase,
    Reset,
    ReadFlash,
    ReadMemory {
        address: u64,
        len: usize,
    },
    WriteMemory {
        address: u64,
        data: Vec<u8>,
    },
    /// 导入自定义芯片（CMSIS-Pack / .pdsc 目录 / 目标描述 YAML）
    ImportTarget {
        path: PathBuf,
    },
    /// 删除已导入的自定义芯片系列
    RemoveTarget {
        name: String,
    },
}

#[derive(Debug)]
pub enum Event {
    Chips(ChipDb),
    Probes(Vec<ProbeId>),
    Log {
        level: LogLevel,
        message: String,
    },
    Started(OpKind),
    Progress {
        fraction: f32,
        text: String,
    },
    Finished {
        kind: OpKind,
        ok: bool,
        summary: String,
    },
    Attached {
        chip: String,
        probe: String,
    },
    Detached,
    TargetInfo {
        flash_start: u64,
        flash_size: u64,
    },
    /// 当前已导入的自定义芯片系列列表
    CustomTargets(Vec<CustomTargetInfo>),
    Dump {
        address: u64,
        data: Vec<u8>,
        label: String,
        kind: DumpKind,
    },
}

// ---------------------------------------------------------------------------
// 句柄
// ---------------------------------------------------------------------------

pub struct WorkerHandle {
    tx: Sender<Command>,
    pub rx: Receiver<Event>,
    abort: Arc<AtomicBool>,
}

impl WorkerHandle {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let abort = Arc::new(AtomicBool::new(false));
        let thread_abort = Arc::clone(&abort);

        std::thread::Builder::new()
            .name("probe-rs-flasher-worker".to_string())
            .spawn(move || {
                let mut worker = Worker::new(event_tx, thread_abort);
                worker.run(rx);
            })
            .expect("无法创建后台工作线程");

        Self {
            tx,
            rx: event_rx,
            abort,
        }
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    pub fn request_abort(&self) {
        self.abort.store(true, Ordering::SeqCst);
    }
}

// ---------------------------------------------------------------------------
// 工作线程
// ---------------------------------------------------------------------------

struct ProgressTracker {
    tx: Sender<Event>,
    totals: [Option<u64>; 4],
    done: [u64; 4],
    started: [Option<Instant>; 4],
}

impl ProgressTracker {
    fn new(tx: Sender<Event>) -> Self {
        Self {
            tx,
            totals: [None; 4],
            done: [0; 4],
            started: [None; 4],
        }
    }

    fn index(op: ProgressOperation) -> usize {
        match op {
            ProgressOperation::Fill => 0,
            ProgressOperation::Erase => 1,
            ProgressOperation::Program => 2,
            ProgressOperation::Verify => 3,
        }
    }

    fn label(op: ProgressOperation) -> &'static str {
        match op {
            ProgressOperation::Fill => t("填充"),
            ProgressOperation::Erase => t("擦除"),
            ProgressOperation::Program => t("烧录"),
            ProgressOperation::Verify => t("校验"),
        }
    }

    fn handle(&mut self, event: ProgressEvent) {
        match event {
            ProgressEvent::FlashLayoutReady { flash_layout } => {
                let sectors: usize = flash_layout.iter().map(|l| l.sectors().len()).sum();
                self.log(LogLevel::Info, tt!("Flash 布局就绪，共 {} 个扇区", sectors));
            }
            ProgressEvent::AddProgressBar { operation, total } => {
                let idx = Self::index(operation);
                self.totals[idx] = total;
                self.done[idx] = 0;
            }
            ProgressEvent::Started(operation) => {
                let idx = Self::index(operation);
                self.started[idx] = Some(Instant::now());
                self.done[idx] = 0;
                self.log(LogLevel::Info, tt!("开始{}…", Self::label(operation)));
                self.progress(0.0, format!("{} 0%", Self::label(operation)));
            }
            ProgressEvent::Progress {
                operation, size, ..
            } => {
                let idx = Self::index(operation);
                self.done[idx] = self.done[idx].saturating_add(size);
                let fraction = match self.totals[idx] {
                    Some(total) if total > 0 => {
                        (self.done[idx] as f32 / total as f32).clamp(0.0, 1.0)
                    }
                    _ => 0.0,
                };
                self.progress(
                    fraction,
                    format!("{} {:.0}%", Self::label(operation), fraction * 100.0),
                );
            }
            ProgressEvent::Finished(operation) => {
                let idx = Self::index(operation);
                let ms = self.started[idx]
                    .take()
                    .map(|t| t.elapsed().as_millis())
                    .unwrap_or(0);
                self.log(
                    LogLevel::Info,
                    tt!("{}完成 {} ms", Self::label(operation), ms),
                );
            }
            ProgressEvent::Failed(operation) => {
                self.log(LogLevel::Error, tt!("{}失败", Self::label(operation)));
            }
            ProgressEvent::DiagnosticMessage { message } => {
                self.log(LogLevel::Info, message);
            }
        }
    }

    fn log(&self, level: LogLevel, message: String) {
        let _ = self.tx.send(Event::Log { level, message });
    }

    fn progress(&self, fraction: f32, text: String) {
        let _ = self.tx.send(Event::Progress { fraction, text });
    }
}

struct Worker {
    tx: Sender<Event>,
    abort: Arc<AtomicBool>,
    session: Option<Session>,
    probes: Vec<DebugProbeInfo>,
    last_probe_summary: String,
    /// 内置 + 自定义芯片的注册表，连接目标时使用
    registry: Option<Registry>,
    custom_targets: Vec<CustomTargetInfo>,
}

impl Worker {
    fn new(tx: Sender<Event>, abort: Arc<AtomicBool>) -> Self {
        Self {
            tx,
            abort,
            session: None,
            probes: Vec::new(),
            last_probe_summary: String::new(),
            registry: None,
            custom_targets: Vec::new(),
        }
    }

    fn run(&mut self, rx: Receiver<Command>) {
        self.log(LogLevel::Info, tt!("Probe-rs Flasher V0.1 已启动"));

        self.reload_targets();
        self.scan_probes();

        loop {
            match rx.recv_timeout(Duration::from_millis(100)) {
                Ok(cmd) => self.handle(cmd),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        self.session = None;
    }

    fn log(&self, level: LogLevel, message: String) {
        let _ = self.tx.send(Event::Log { level, message });
    }

    fn aborted(&self) -> bool {
        self.abort.load(Ordering::SeqCst)
    }

    fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::RefreshProbes => self.scan_probes(),
            Command::Attach {
                probe,
                chip,
                protocol,
                speed_khz,
                reset,
                quiet,
            } => {
                self.exec(OpKind::Connect, quiet, |w| {
                    w.do_attach(probe, chip, protocol, speed_khz, reset)
                });
            }
            Command::Flash {
                path,
                format,
                base_address,
                chip_erase,
                verify,
            } => {
                self.exec(OpKind::Flash, false, |w| {
                    w.do_flash(path, format, base_address, chip_erase, verify)
                });
            }
            Command::Verify {
                path,
                format,
                base_address,
            } => {
                self.exec(OpKind::Verify, false, |w| {
                    w.do_verify(path, format, base_address)
                });
            }
            Command::Erase => {
                self.exec(OpKind::Erase, false, |w| w.do_erase());
            }
            Command::Reset => {
                self.exec(OpKind::Reset, false, |w| w.do_reset());
            }
            Command::ReadFlash => {
                self.exec(OpKind::ReadFlash, false, |w| w.do_read_flash());
            }
            Command::ReadMemory { address, len } => {
                self.exec(OpKind::ReadMemory, false, |w| {
                    w.do_read_memory(address, len)
                });
            }
            Command::WriteMemory { address, data } => {
                self.exec(OpKind::WriteMemory, false, |w| {
                    w.do_write_memory(address, data)
                });
            }
            Command::ImportTarget { path } => {
                self.exec(OpKind::ImportTarget, false, move |w| {
                    w.do_import_target(path)
                });
            }
            Command::RemoveTarget { name } => {
                self.exec(OpKind::RemoveTarget, false, move |w| {
                    w.do_remove_target(name)
                });
            }
        }
    }

    fn exec<F>(&mut self, kind: OpKind, quiet: bool, f: F)
    where
        F: FnOnce(&mut Self) -> Result<String, String>,
    {
        self.abort.store(false, Ordering::SeqCst);
        let _ = self.tx.send(Event::Started(kind));
        let _ = self.tx.send(Event::Progress {
            fraction: 0.0,
            text: tt!("{}中…", kind.label()),
        });

        let result = f(self);

        let _ = self.tx.send(Event::Progress {
            fraction: 0.0,
            text: String::new(),
        });

        match result {
            Ok(summary) => {
                self.log(LogLevel::Ok, summary.clone());
                let _ = self.tx.send(Event::Finished {
                    kind,
                    ok: true,
                    summary,
                });
            }
            Err(message) => {
                let message = if self.aborted() {
                    tt!("{}已终止", kind.label())
                } else {
                    message
                };
                if !quiet {
                    self.log(LogLevel::Error, message.clone());
                }
                let _ = self.tx.send(Event::Finished {
                    kind,
                    ok: false,
                    summary: message,
                });
            }
        }
    }

    // -----------------------------------------------------------------------
    // 调试器 / 连接
    // -----------------------------------------------------------------------

    fn scan_probes(&mut self) {
        let lister = Lister::new();
        let probes = lister.list_all();
        let summary = if probes.is_empty() {
            tt!("未检测到调试器")
        } else {
            let names: Vec<String> = probes
                .iter()
                .map(|p| match &p.serial_number {
                    Some(serial) if !serial.is_empty() => {
                        format!("{} ({})", p.identifier, serial)
                    }
                    _ => p.identifier.clone(),
                })
                .collect();
            tt!("检测到 {} 个调试器: {}", probes.len(), names.join(", "))
        };

        if summary != self.last_probe_summary {
            self.log(LogLevel::Info, summary.clone());
            self.last_probe_summary = summary;
        }

        let ids: Vec<ProbeId> = probes.iter().map(ProbeId::from).collect();
        self.probes = probes;
        let _ = self.tx.send(Event::Probes(ids));
    }

    // -----------------------------------------------------------------------
    // 自定义芯片（target-gen 导入）
    // -----------------------------------------------------------------------

    /// 重新加载内置 + 自定义芯片，构建注册表并通知 UI。
    fn reload_targets(&mut self) {
        let start = Instant::now();
        let mut registry = Registry::from_builtin_families();
        let mut custom_targets = Vec::new();
        let mut custom_names = Vec::new();

        for (path, yaml) in custom::load_saved_yamls() {
            let chips = custom::parse_family_yaml(&yaml)
                .map(|family| family.variants.len())
                .unwrap_or(0);
            match registry.add_target_family_from_yaml(&yaml) {
                Ok(name) => {
                    custom_names.push(name.clone());
                    custom_targets.push(CustomTargetInfo { name, path, chips });
                }
                Err(e) => {
                    self.log(
                        LogLevel::Warn,
                        tt!("跳过自定义芯片 {}: {}", path.display(), e),
                    );
                }
            }
        }

        let db = ChipDb::from_registry_with_custom(&registry, &custom_names);
        let families = db.total_families();
        let chips = db.total_chips();

        let custom_note = if custom_targets.is_empty() {
            String::new()
        } else {
            tt!("，含 {} 个自定义系列", custom_targets.len())
        };
        self.log(
            LogLevel::Info,
            tt!(
                "芯片库加载完成: {} 个系列 / {} 个型号{} ({} ms)",
                families,
                chips,
                custom_note,
                start.elapsed().as_millis()
            ),
        );

        self.registry = Some(registry);
        self.custom_targets = custom_targets.clone();

        let _ = self.tx.send(Event::Chips(db));
        let _ = self.tx.send(Event::CustomTargets(custom_targets));
    }

    fn do_import_target(&mut self, path: PathBuf) -> Result<String, String> {
        let start = Instant::now();
        self.log(
            LogLevel::Info,
            tt!("正在解析自定义芯片文件 {} …", path.display()),
        );

        let families = custom::generate_from_path(&path)?;
        let total_chips: usize = families.iter().map(|f| f.variants.len()).sum();
        let saved = custom::save_families(&families)?;

        for entry in &saved {
            self.log(
                LogLevel::Info,
                tt!(
                    "已保存自定义芯片: {} ({} 个型号) -> {}",
                    entry.name,
                    entry.chips,
                    entry.path.display()
                ),
            );
        }

        let family_count = saved.len();
        self.reload_targets();

        Ok(tt!(
            "导入完成 {} ms, {} 个系列 / {} 个型号",
            start.elapsed().as_millis(),
            family_count,
            total_chips
        ))
    }

    fn do_remove_target(&mut self, name: String) -> Result<String, String> {
        let target = self
            .custom_targets
            .iter()
            .find(|t| t.name == name)
            .cloned()
            .ok_or_else(|| tt!("未找到自定义芯片系列: {}", name))?;

        std::fs::remove_file(&target.path)
            .map_err(|e| tt!("删除 {} 失败: {}", target.path.display(), e))?;

        self.reload_targets();

        Ok(tt!("已删除自定义芯片: {} ({} 个型号)", name, target.chips))
    }

    fn do_attach(
        &mut self,
        probe: Option<ProbeId>,
        chip: String,
        protocol: Protocol,
        speed_khz: u32,
        reset: ResetMode,
    ) -> Result<String, String> {
        self.session = None;
        let _ = self.tx.send(Event::Detached);

        let lister = Lister::new();
        let mut debug_probe = match &probe {
            Some(id) => lister
                .open(id.selector())
                .map_err(|e| tt!("打开调试器 {} 失败: {}", id.name, e))?,
            None => {
                let first = self
                    .probes
                    .first()
                    .cloned()
                    .ok_or_else(|| tt!("未检测到调试器，请检查 USB 连接与驱动"))?;
                lister
                    .open(ProbeId::from(&first).selector())
                    .map_err(|e| tt!("打开调试器 {} 失败: {}", first.identifier, e))?
            }
        };

        let probe_name = debug_probe.get_name();

        debug_probe
            .select_protocol(protocol.wire())
            .map_err(|e| tt!("切换到 {} 失败: {}", protocol.label(), e))?;

        match debug_probe.set_speed(speed_khz) {
            Ok(actual) => {
                if actual != speed_khz {
                    self.log(
                        LogLevel::Warn,
                        tt!(
                            "调试器不支持 {} MHz，已使用 {} kHz",
                            speed_khz as f64 / 1000.0,
                            actual
                        ),
                    );
                }
            }
            Err(e) => self.log(
                LogLevel::Warn,
                tt!("设置速率失败（继续使用默认速率）: {}", e),
            ),
        }

        let permissions = Permissions::new().allow_erase_all();

        if self.registry.is_none() {
            self.registry = Some(Registry::from_builtin_families());
        }
        let registry = self.registry.as_ref().expect("registry 已经在启动时初始化");

        // 使用同时包含内置芯片与自定义芯片的注册表连接目标
        let mut session = match reset {
            ResetMode::Default => {
                debug_probe.attach_with_registry(chip.as_str(), permissions, registry)
            }
            ResetMode::UnderReset => {
                debug_probe.attach_under_reset_with_registry(chip.as_str(), permissions, registry)
            }
            ResetMode::HardReset => {
                let mut p = debug_probe;
                let _ = p.target_reset();
                p.attach_with_registry(chip.as_str(), permissions, registry)
            }
        }
        .map_err(|e| tt!("连接目标芯片 {} 失败: {}", chip, e))?;

        // 汇报 Flash 起始地址/容量，供 bin 文件定位。
        if let Some((start, size)) = nvm_summary(&session) {
            let _ = self.tx.send(Event::TargetInfo {
                flash_start: start,
                flash_size: size,
            });
        }

        // 保持内核停止，避免烧录过程中目标跑飞。
        if let Ok(mut core) = session.core(0) {
            let _ = core.halt(Duration::from_millis(200));
        }

        self.session = Some(session);

        let _ = self.tx.send(Event::Attached {
            chip: chip.clone(),
            probe: probe_name.clone(),
        });

        Ok(tt!("已连接目标 {} · {}", chip, probe_name))
    }

    /// 直接取得 session 字段，避免与 `self.tx` / `self.abort` 的借用冲突。
    fn session_slot(&mut self) -> Result<&mut Session, String> {
        self.session
            .as_mut()
            .ok_or_else(|| tt!("未连接目标，请先连接调试器"))
    }

    // -----------------------------------------------------------------------
    // 烧录 / 校验 / 擦除 / 复位
    // -----------------------------------------------------------------------

    fn progress(&self) -> FlashProgress<'static> {
        let mut tracker = ProgressTracker::new(self.tx.clone());
        FlashProgress::new(move |event| tracker.handle(event))
    }

    fn do_flash(
        &mut self,
        path: PathBuf,
        format: FirmwareFormat,
        base_address: u64,
        chip_erase: bool,
        verify: bool,
    ) -> Result<String, String> {
        let start = Instant::now();
        let image = make_loader(format, base_address);
        let progress = self.progress();
        let session = self.session_slot()?;

        let loader =
            build_loader(session, &path, image, None).map_err(|e| tt!("解析固件失败: {}", e))?;
        let total: u64 = loader.data().map(|(_, d)| d.len() as u64).sum();

        let mut options = DownloadOptions::default();
        options.progress = progress;
        options.do_chip_erase = chip_erase;
        options.keep_unwritten_bytes = !chip_erase;
        options.verify = verify;
        options.preverify = false;
        options.skip_erase = false;
        options.dry_run = false;

        loader
            .commit(session, options)
            .map_err(|e| tt!("烧录失败: {}", e))?;

        Ok(tt!(
            "烧录完成 {} ms, 共 {}",
            start.elapsed().as_millis(),
            format_bytes(total)
        ))
    }

    fn do_verify(
        &mut self,
        path: PathBuf,
        format: FirmwareFormat,
        base_address: u64,
    ) -> Result<String, String> {
        let start = Instant::now();
        let image = make_loader(format, base_address);
        let mut progress = self.progress();
        let session = self.session_slot()?;

        let loader =
            build_loader(session, &path, image, None).map_err(|e| tt!("解析固件失败: {}", e))?;

        loader
            .verify(session, &mut progress)
            .map_err(|e| tt!("校验失败: {}", e))?;

        Ok(tt!("校验完成 {} ms", start.elapsed().as_millis()))
    }

    fn do_erase(&mut self) -> Result<String, String> {
        let start = Instant::now();
        let mut progress = self.progress();
        let session = self.session_slot()?;

        flashing::erase_all(session, &mut progress, false).map_err(|e| tt!("擦除失败: {}", e))?;

        Ok(tt!("擦除完成 {} ms", start.elapsed().as_millis()))
    }

    fn do_reset(&mut self) -> Result<String, String> {
        let start = Instant::now();
        let session = self.session_slot()?;
        let mut core = session.core(0).map_err(|e| tt!("无法访问内核: {}", e))?;
        core.reset().map_err(|e| tt!("复位失败: {}", e))?;

        Ok(tt!("复位完成 {} ms", start.elapsed().as_millis()))
    }

    // -----------------------------------------------------------------------
    // 读取 Flash / 内存读写
    // -----------------------------------------------------------------------

    fn do_read_flash(&mut self) -> Result<String, String> {
        let start = Instant::now();
        let tx = self.tx.clone();
        let abort = Arc::clone(&self.abort);
        let session = self.session_slot()?;

        let regions: Vec<(u64, u64)> = session
            .target()
            .memory_map
            .iter()
            .filter_map(|r| r.as_nvm_region())
            .filter(|r| !r.is_alias)
            .map(|r| (r.range.start, r.range.end))
            .collect();

        if regions.is_empty() {
            return Err(tt!("目标芯片没有可读取的 Flash 区域"));
        }

        let total_bytes: u64 = regions.iter().map(|(s, e)| e.saturating_sub(*s)).sum();
        let mut dumps: Vec<(u64, Vec<u8>)> = Vec::new();
        let mut base_done = 0u64;

        for (region_start, region_end) in regions {
            let len = region_end.saturating_sub(region_start) as usize;
            if len == 0 {
                continue;
            }
            let data = read_memory(session, region_start, len, &abort, |done, _len| {
                let fraction =
                    ((base_done + done as u64) as f32 / total_bytes as f32).clamp(0.0, 1.0);
                let _ = tx.send(Event::Progress {
                    fraction,
                    text: tt!("读取 {}%", format!("{:.0}", fraction * 100.0)),
                });
            })?;
            base_done += len as u64;
            dumps.push((region_start, data));
        }

        let (address, data) = dumps
            .into_iter()
            .max_by_key(|(_, d)| d.len())
            .ok_or_else(|| tt!("没有读取到数据"))?;

        let _ = self.tx.send(Event::Dump {
            address,
            label: tt!("目标内存"),
            data: data.clone(),
            kind: DumpKind::Target,
        });

        Ok(tt!(
            "读取完成 {} ms, 起始 0x{}, 共 {}",
            start.elapsed().as_millis(),
            format!("{address:08X}"),
            format_bytes(data.len() as u64)
        ))
    }

    fn do_read_memory(&mut self, address: u64, len: usize) -> Result<String, String> {
        let start = Instant::now();
        let len = len.min(crate::firmware::MAX_VIEW_BYTES);
        let tx = self.tx.clone();
        let abort = Arc::clone(&self.abort);
        let session = self.session_slot()?;

        let data = read_memory(session, address, len, &abort, |done, total| {
            let fraction = (done as f32 / total.max(1) as f32).clamp(0.0, 1.0);
            let _ = tx.send(Event::Progress {
                fraction,
                text: tt!("读取 {}%", format!("{:.0}", fraction * 100.0)),
            });
        })?;

        let _ = self.tx.send(Event::Dump {
            address,
            data: data.clone(),
            label: tt!("内存"),
            kind: DumpKind::MemoryWindow,
        });

        Ok(tt!(
            "读取内存完成 {} ms, 0x{} 起 {} 字节",
            start.elapsed().as_millis(),
            format!("{address:08X}"),
            data.len()
        ))
    }

    fn do_write_memory(&mut self, address: u64, data: Vec<u8>) -> Result<String, String> {
        let start = Instant::now();
        let session = self.session_slot()?;
        let mut core = session.core(0).map_err(|e| tt!("无法访问内核: {}", e))?;

        let was_halted = core.core_halted().unwrap_or(false);
        if !was_halted {
            let _ = core.halt(Duration::from_millis(200));
        }

        let result = core.write(address, &data);
        if !was_halted {
            let _ = core.run();
        }
        result.map_err(|e| tt!("写入内存失败 @ 0x{}: {}", format!("{address:08X}"), e))?;

        Ok(tt!(
            "写入内存完成 {} ms, 0x{} 起 {} 字节",
            start.elapsed().as_millis(),
            format!("{address:08X}"),
            data.len()
        ))
    }
}

// ---------------------------------------------------------------------------
// 辅助函数
// ---------------------------------------------------------------------------

fn make_loader(format: FirmwareFormat, base_address: u64) -> Box<dyn ImageLoader> {
    match format {
        FirmwareFormat::Bin => Box::new(BinLoader(BinOptions {
            base_address: Some(base_address),
            skip: 0,
        })),
        FirmwareFormat::Hex => Box::new(HexLoader),
        FirmwareFormat::Elf => Box::new(ElfLoader(ElfOptions::default())),
        FirmwareFormat::Uf2 => Box::new(Uf2Loader),
    }
}

fn nvm_summary(session: &Session) -> Option<(u64, u64)> {
    let mut start = u64::MAX;
    let mut size = 0u64;
    for region in session
        .target()
        .memory_map
        .iter()
        .filter_map(|r| r.as_nvm_region())
        .filter(|r| !r.is_alias)
    {
        start = start.min(region.range.start);
        size += region.range.end.saturating_sub(region.range.start);
    }
    (start != u64::MAX).then_some((start, size))
}

fn read_memory(
    session: &mut Session,
    address: u64,
    len: usize,
    abort: &AtomicBool,
    mut on_progress: impl FnMut(usize, usize),
) -> Result<Vec<u8>, String> {
    let mut core = session.core(0).map_err(|e| tt!("无法访问内核: {}", e))?;
    let was_halted = core.core_halted().unwrap_or(false);
    if !was_halted {
        let _ = core.halt(Duration::from_millis(200));
    }

    let mut out = Vec::with_capacity(len);
    let mut buffer = vec![0u8; 4096];
    let mut offset = 0usize;

    while offset < len {
        if abort.load(Ordering::SeqCst) {
            if !was_halted {
                let _ = core.run();
            }
            return Err(tt!("操作已终止"));
        }
        let chunk = (len - offset).min(buffer.len());
        let current = address + offset as u64;
        if let Err(e) = core.read(current, &mut buffer[..chunk]) {
            if !was_halted {
                let _ = core.run();
            }
            return Err(tt!("读取内存失败 @ 0x{}: {}", format!("{current:08X}"), e));
        }
        out.extend_from_slice(&buffer[..chunk]);
        offset += chunk;
        on_progress(offset, len);
    }

    if !was_halted {
        let _ = core.run();
    }
    Ok(out)
}
