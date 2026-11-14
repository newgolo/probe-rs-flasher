//! 界面：Probe-rs Flasher 的主界面布局与交互。

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Id, Layout, Rect, RichText, ScrollArea, Sense,
    Stroke, Ui, ViewportCommand, pos2, vec2,
};

use crate::chips::ChipDb;
use crate::custom::{self, CUSTOM_VENDOR, CustomTargetInfo};
use crate::firmware::{self, FirmwareFormat, LoadedFirmware, format_bytes};
use crate::i18n::{self, Lang, t};
use crate::theme::{
    self, ACCENT, ACCENT_SOFT, BG, BORDER_SOFT, DROP_BORDER, DROP_BORDER_HOT, DROP_FILL,
    DROP_FILL_HOT, GREEN, HEX_ADDRESS, HEX_ASCII, HEX_BYTE, LABEL, MUTED, ORANGE, RED, TEXT,
    TITLE_BG,
};
use crate::tt;
use crate::worker::{
    Command, DumpKind, Event, LogLevel, OpKind, ProbeId, Protocol, ResetMode, WorkerHandle,
};

// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------

const SPEEDS: &[(&str, u32)] = &[
    ("1 MHz", 1_000),
    ("2 MHz", 2_000),
    ("5 MHz", 5_000),
    ("10 MHz", 10_000),
    ("20 MHz", 20_000),
    ("40 MHz", 40_000),
    ("50 MHz", 50_000),
];

const RESET_MODES: &[(&str, ResetMode)] = &[
    ("默认", ResetMode::Default),
    ("连接时复位", ResetMode::UnderReset),
    ("硬件复位", ResetMode::HardReset),
];

/// 标题栏显示的版本号。
const VERSION_LABEL: &str = "V0.1";
/// 项目主页。
const PROJECT_URL: &str = "https://github.com/newgolo/probe-rs-flasher";
/// 标题栏显示的链接文本。
const PROJECT_LINK: &str = "github.com/newgolo/probe-rs-flasher";

const DEFAULT_FLASH_BASE: u64 = 0x0800_0000;
const LOG_LIMIT: usize = 1200;

// ---------------------------------------------------------------------------
// 数据结构
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum HexSource {
    Firmware,
    Target,
}

struct HexView {
    base: u64,
    data: Vec<u8>,
    label: String,
    source: HexSource,
    crc32: u32,
}

#[derive(Clone)]
struct LogLine {
    time: String,
    message: String,
    level: LogLevel,
}

struct MemDialog {
    open: bool,
    address: String,
    length: String,
    read_result: String,
    write_data: String,
    status: Option<String>,
    error: Option<String>,
}

impl Default for MemDialog {
    fn default() -> Self {
        Self {
            open: false,
            address: "0x20000000".to_string(),
            length: "256".to_string(),
            read_result: String::new(),
            write_data: String::new(),
            status: None,
            error: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PickerMode {
    OpenFirmware,
    SaveDump,
    /// 导入自定义芯片（CMSIS-Pack / .pdsc / 目标描述 YAML）
    OpenTarget,
}

struct PickerEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
}

struct FilePicker {
    mode: PickerMode,
    dir: PathBuf,
    entries: Vec<PickerEntry>,
    file_name: String,
    path_input: String,
    error: Option<String>,
    show_all: bool,
}

// ---------------------------------------------------------------------------
// 主应用
// ---------------------------------------------------------------------------

pub struct FlasherApp {
    worker: WorkerHandle,

    // 芯片库
    chips: Option<ChipDb>,
    vendor: usize,
    family: usize,
    chip: usize,
    search: String,

    // 连接
    probes: Vec<ProbeId>,
    probe: Option<ProbeId>,
    protocol: Protocol,
    speed_index: usize,
    reset_index: usize,
    connected_chip: Option<String>,
    connected_probe: Option<String>,
    flash_base: u64,
    flash_size: u64,

    // 操作状态
    busy: Option<OpKind>,
    progress: f32,
    progress_text: String,
    status_message: String,
    status_ok: bool,

    // 固件 / 视图
    firmware: Option<LoadedFirmware>,
    view: Option<HexView>,

    // 日志
    logs: Vec<LogLine>,
    follow_log: bool,

    // 选项
    opt_chip_erase: bool,
    opt_auto_flash: bool,
    opt_auto_connect: bool,

    // 自动任务
    pending: Vec<Command>,
    last_probe_scan: Instant,
    last_auto_connect: Instant,
    auto_flash_pending: bool,

    // 对话框
    picker: Option<FilePicker>,
    mem: MemDialog,

    // 自定义芯片
    custom_targets: Vec<CustomTargetInfo>,
    custom_dialog_open: bool,
    custom_delete_confirm: Option<String>,
    select_custom_after_import: bool,

    // 调试用：环境变量 PROBE_RS_FLASHER_SHOT 指定路径时，渲染 30 帧后输出 PPM 截图。
    shot_path: Option<PathBuf>,
    shot_frames: u32,
}

impl FlasherApp {
    pub fn new(cc: &eframe::CreationContext<'_>, initial_file: Option<PathBuf>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply_style(&cc.egui_ctx);

        let now = Instant::now();
        let mut app = Self {
            worker: WorkerHandle::spawn(),
            chips: None,
            vendor: 0,
            family: 0,
            chip: 0,
            search: String::new(),
            probes: Vec::new(),
            probe: None,
            protocol: Protocol::Swd,
            speed_index: 3,
            reset_index: 0,
            connected_chip: None,
            connected_probe: None,
            flash_base: DEFAULT_FLASH_BASE,
            flash_size: 0,
            busy: None,
            progress: 0.0,
            progress_text: String::new(),
            status_message: tt!("就绪"),
            status_ok: true,
            firmware: None,
            view: None,
            logs: Vec::new(),
            follow_log: true,
            opt_chip_erase: true,
            opt_auto_flash: false,
            opt_auto_connect: true,
            pending: Vec::new(),
            last_probe_scan: now - Duration::from_secs(10),
            last_auto_connect: now - Duration::from_secs(10),
            auto_flash_pending: false,
            picker: None,
            mem: MemDialog::default(),
            custom_targets: Vec::new(),
            custom_dialog_open: false,
            custom_delete_confirm: None,
            select_custom_after_import: false,
            shot_path: std::env::var_os("PROBE_RS_FLASHER_SHOT").map(PathBuf::from),
            shot_frames: 0,
        };
        app.push_log(LogLevel::Info, tt!("正在初始化，请稍候…"));
        if let Some(path) = initial_file {
            app.load_firmware_file(path);
        }
        app
    }

    // -----------------------------------------------------------------------
    // 日志
    // -----------------------------------------------------------------------

    fn push_log(&mut self, level: LogLevel, message: String) {
        let time = chrono::Local::now().format("%H:%M:%S").to_string();
        self.logs.push(LogLine {
            time,
            message,
            level,
        });
        if self.logs.len() > LOG_LIMIT {
            let overflow = self.logs.len() - LOG_LIMIT;
            self.logs.drain(..overflow);
        }
    }

    // -----------------------------------------------------------------------
    // 事件
    // -----------------------------------------------------------------------

    fn handle_events(&mut self) {
        while let Ok(event) = self.worker.rx.try_recv() {
            match event {
                Event::Chips(db) => {
                    let previous = self.selected_chip_name();
                    let prefer_custom = std::mem::take(&mut self.select_custom_after_import);
                    self.chips = Some(db);

                    if prefer_custom {
                        if let Some((v, f, c)) = self.first_custom_selection() {
                            self.vendor = v;
                            self.family = f;
                            self.chip = c;
                            if let Some(name) =
                                self.chips.as_ref().and_then(|db| db.chip_name(v, f, c))
                            {
                                self.push_log(LogLevel::Info, tt!("已切换到自定义芯片: {}", name));
                            }
                        }
                    } else if previous.is_none() {
                        if let Some((v, f, c)) =
                            self.chips.as_ref().and_then(|db| db.default_selection())
                        {
                            self.vendor = v;
                            self.family = f;
                            self.chip = c;
                            if let Some(name) =
                                self.chips.as_ref().and_then(|db| db.chip_name(v, f, c))
                            {
                                self.push_log(LogLevel::Info, tt!("默认芯片型号: {}", name));
                            }
                        }
                    } else if let Some(name) = previous {
                        // 重新加载芯片库（导入/删除自定义芯片）后尽量保持当前选择
                        let found = self.chips.as_ref().and_then(|db| db.find(&name));
                        match found {
                            Some((v, f, c)) => {
                                self.vendor = v;
                                self.family = f;
                                self.chip = c;
                            }
                            None => {
                                if let Some((v, f, c)) =
                                    self.chips.as_ref().and_then(|db| db.default_selection())
                                {
                                    self.vendor = v;
                                    self.family = f;
                                    self.chip = c;
                                }
                            }
                        }
                    }
                }
                Event::CustomTargets(list) => {
                    self.custom_targets = list;
                    if let Some(name) = &self.custom_delete_confirm
                        && !self.custom_targets.iter().any(|t| &t.name == name)
                    {
                        self.custom_delete_confirm = None;
                    }
                }
                Event::Probes(list) => {
                    let changed = list != self.probes;
                    self.probes = list;
                    if let Some(current) = &self.probe {
                        if !self.probes.iter().any(|p| p == current) {
                            self.probe = self.probes.first().cloned();
                        }
                    } else {
                        self.probe = self.probes.first().cloned();
                    }
                    if changed && self.probes.is_empty() {
                        self.connected_chip = None;
                        self.connected_probe = None;
                    }
                }
                Event::Log { level, message } => self.push_log(level, message),
                Event::Started(kind) => {
                    self.busy = Some(kind);
                    self.progress = 0.0;
                    self.progress_text = tt!("{}中…", kind.label());
                }
                Event::Progress { fraction, text } => {
                    self.progress = fraction;
                    if !text.is_empty() {
                        self.progress_text = text;
                    }
                }
                Event::Finished { kind, ok, summary } => {
                    self.busy = None;
                    self.progress = 0.0;
                    self.status_ok = ok;
                    self.status_message = summary;
                    if kind == OpKind::Connect && ok {
                        self.auto_flash_pending = self.opt_auto_flash && self.firmware.is_some();
                    }
                    if kind == OpKind::ImportTarget && !ok {
                        self.select_custom_after_import = false;
                    }
                }
                Event::Attached { chip, probe } => {
                    self.connected_chip = Some(chip);
                    self.connected_probe = Some(probe);
                }
                Event::Detached => {
                    self.connected_chip = None;
                    self.connected_probe = None;
                }
                Event::TargetInfo {
                    flash_start,
                    flash_size,
                } => {
                    self.flash_base = flash_start;
                    self.flash_size = flash_size;

                    if let Some(fw) = &mut self.firmware
                        && fw.format == FirmwareFormat::Bin
                    {
                        fw.base = flash_start;
                        fw.load_base = flash_start;
                        if let Some(view) = &mut self.view
                            && view.source == HexSource::Firmware
                        {
                            view.base = flash_start;
                        }
                        self.push_log(
                            LogLevel::Info,
                            tt!(
                                "bin 固件基地址: 0x{} (Flash {})",
                                format!("{flash_start:08X}"),
                                format_bytes(flash_size)
                            ),
                        );
                    }
                }
                Event::Dump {
                    address,
                    data,
                    label,
                    kind,
                } => {
                    let crc32 = crc32fast::hash(&data);
                    match kind {
                        DumpKind::Target => {
                            self.view = Some(HexView {
                                base: address,
                                data,
                                label,
                                source: HexSource::Target,
                                crc32,
                            });
                        }
                        DumpKind::MemoryWindow => {
                            self.mem.read_result = firmware::hexdump_text(address, &data, 4096);
                            self.mem.status =
                                Some(tt!("已读取 {} 字节", format_bytes(data.len() as u64)));
                            self.mem.error = None;
                        }
                    }
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 周期任务
    // -----------------------------------------------------------------------

    fn tick(&mut self) {
        if self.busy.is_none() && self.last_probe_scan.elapsed() > Duration::from_secs(2) {
            self.last_probe_scan = Instant::now();
            self.pending.push(Command::RefreshProbes);
        }

        if self.auto_flash_pending
            && self.busy.is_none()
            && self.connected_chip.is_some()
            && self.firmware.is_some()
        {
            self.auto_flash_pending = false;
            self.push_log(LogLevel::Info, tt!("自动烧录已触发"));
            self.request_flash();
        }

        let should_auto_connect = self.opt_auto_connect
            && self.busy.is_none()
            && self.connected_chip.is_none()
            && self.chips.is_some()
            && !self.probes.is_empty()
            && self.last_auto_connect.elapsed() > Duration::from_secs(8);

        if should_auto_connect && let Some(chip) = self.selected_chip_name() {
            self.last_auto_connect = Instant::now();
            self.pending.push(Command::Attach {
                probe: self.probe.clone(),
                chip,
                protocol: self.protocol,
                speed_khz: self.speed_khz(),
                reset: self.reset_mode(),
                quiet: true,
            });
        }
    }

    // -----------------------------------------------------------------------
    // 便捷访问
    // -----------------------------------------------------------------------

    fn speed_khz(&self) -> u32 {
        SPEEDS
            .get(self.speed_index)
            .map(|(_, khz)| *khz)
            .unwrap_or(10_000)
    }

    fn reset_mode(&self) -> ResetMode {
        RESET_MODES
            .get(self.reset_index)
            .map(|(_, mode)| *mode)
            .unwrap_or(ResetMode::Default)
    }

    fn selected_chip_name(&self) -> Option<String> {
        self.chips
            .as_ref()
            .and_then(|db| db.chip_name(self.vendor, self.family, self.chip))
    }

    fn flush_pending(&mut self) {
        for cmd in self.pending.drain(..) {
            self.worker.send(cmd);
        }
    }

    fn reconnect(&mut self) {
        if self.connected_chip.is_none() {
            return;
        }
        if let Some(chip) = self.selected_chip_name() {
            self.pending.push(Command::Attach {
                probe: self.probe.clone(),
                chip,
                protocol: self.protocol,
                speed_khz: self.speed_khz(),
                reset: self.reset_mode(),
                quiet: false,
            });
        }
    }

    fn request_flash(&mut self) {
        let Some(fw) = &self.firmware else {
            self.push_log(LogLevel::Error, tt!("请先加载固件文件"));
            return;
        };
        if self.connected_chip.is_none() {
            self.push_log(LogLevel::Error, tt!("未连接目标芯片，无法烧录"));
            return;
        }
        if self.busy.is_some() {
            return;
        }
        let path = fw.path.clone();
        let format = fw.format;
        let base = fw.load_base;
        let name = fw.name.clone();
        self.push_log(
            LogLevel::Info,
            tt!("***************** 烧录 {} *****************", name),
        );
        self.pending.push(Command::Flash {
            path,
            format,
            base_address: base,
            chip_erase: self.opt_chip_erase,
            verify: true,
        });
    }

    fn request_verify(&mut self) {
        let Some((path, format, base)) = self
            .firmware
            .as_ref()
            .map(|fw| (fw.path.clone(), fw.format, fw.load_base))
        else {
            self.push_log(LogLevel::Error, tt!("请先加载固件文件"));
            return;
        };
        if self.connected_chip.is_none() || self.busy.is_some() {
            return;
        }
        self.push_log(
            LogLevel::Info,
            tt!("***************** 校验 *****************"),
        );
        self.pending.push(Command::Verify {
            path,
            format,
            base_address: base,
        });
    }

    // -----------------------------------------------------------------------
    // 固件加载
    // -----------------------------------------------------------------------

    fn load_firmware_file(&mut self, path: PathBuf) {
        let start = Instant::now();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        self.push_log(LogLevel::Info, tt!("正在打开 {} …", name));

        match firmware::load_firmware(&path, self.flash_base) {
            Ok(fw) => {
                self.push_log(
                    LogLevel::Info,
                    tt!("已打开 {}: {}", fw.format.label(), fw.path.display()),
                );
                self.push_log(LogLevel::Info, format!("CRC32: 0x{:08X}", fw.crc32));
                self.push_log(
                    LogLevel::Info,
                    tt!(
                        "合计 {}, 起始 0x{}, 视图显示 {}, 耗时 {} ms",
                        fw.size_text(),
                        format!("{:08X}", fw.base),
                        format_bytes(fw.bytes.len() as u64),
                        start.elapsed().as_millis()
                    ),
                );

                self.view = Some(HexView {
                    base: fw.base,
                    data: fw.bytes.clone(),
                    label: fw.name.clone(),
                    source: HexSource::Firmware,
                    crc32: fw.crc32,
                });

                self.status_ok = true;
                self.status_message = tt!("已加载 {}", fw.name);
                self.firmware = Some(fw);
            }
            Err(message) => {
                self.push_log(LogLevel::Error, message.clone());
                self.status_ok = false;
                self.status_message = message;
            }
        }
    }

    fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if let Some(file) = dropped.into_iter().next()
            && let Some(path) = file.path
        {
            self.load_firmware_file(path);
        }
    }

    // -----------------------------------------------------------------------
    // 标题栏
    // -----------------------------------------------------------------------

    fn frame_title_bar(&mut self, ui: &mut Ui, ctx: &egui::Context, rect: Rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::ZERO, TITLE_BG);
        painter.rect_filled(
            Rect::from_min_size(
                pos2(rect.left(), rect.bottom() - 1.0),
                vec2(rect.width(), 1.0),
            ),
            CornerRadius::ZERO,
            BORDER_SOFT,
        );

        // 窗口按钮位置（先确定，供拖拽区域与语言开关让位）
        let btn_w = 46.0;
        let close_rect = Rect::from_min_size(
            pos2(rect.right() - btn_w, rect.top()),
            vec2(btn_w, rect.height()),
        );
        let max_rect = close_rect.translate(vec2(-btn_w, 0.0));
        let min_rect = max_rect.translate(vec2(-btn_w, 0.0));

        // 中英文切换开关（位于窗口按钮左侧）
        let toggle_w = 58.0;
        let toggle_h = 22.0;
        let toggle_rect = Rect::from_center_size(
            pos2(min_rect.left() - 12.0 - toggle_w * 0.5, rect.center().y),
            vec2(toggle_w, toggle_h),
        );

        // 拖拽移动窗口
        let drag_rect = Rect::from_min_max(rect.min, pos2(toggle_rect.left() - 6.0, rect.bottom()));
        let drag = ui.interact(
            drag_rect,
            Id::new("title_bar_drag"),
            Sense::click_and_drag(),
        );
        if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }

        // 图标 + 标题 + 版本 + 项目主页
        let logo_center = pos2(rect.left() + 19.0, rect.center().y);
        theme::paint_icon(painter, logo_center, 17.0, theme::Icon::Download, ACCENT);

        let title_font = FontId::proportional(15.0);
        let title_galley = painter.layout_no_wrap("Probe-rs Flasher".to_string(), title_font, TEXT);
        let title_pos = pos2(
            rect.left() + 36.0,
            rect.center().y - title_galley.size().y * 0.5,
        );
        let version_x = title_pos.x + title_galley.size().x + 8.0;
        painter.galley(title_pos, title_galley, TEXT);

        let meta_font = FontId::proportional(11.0);
        let version_galley =
            painter.layout_no_wrap(VERSION_LABEL.to_string(), meta_font.clone(), MUTED);
        let version_pos = pos2(version_x, rect.center().y - version_galley.size().y * 0.5);
        painter.galley(version_pos, version_galley.clone(), MUTED);

        let link_x = version_x + version_galley.size().x + 14.0;
        let link_size = painter
            .layout_no_wrap(PROJECT_LINK.to_string(), meta_font.clone(), MUTED)
            .size();
        let link_pos = pos2(link_x, rect.center().y - link_size.y * 0.5);
        let link_rect = Rect::from_min_size(link_pos, link_size);
        let link_resp = ui.interact(
            link_rect.expand2(vec2(3.0, 4.0)),
            Id::new("title_project_link"),
            Sense::click(),
        );
        let link_color = if link_resp.hovered() { ACCENT } else { MUTED };
        let link_galley = painter.layout_no_wrap(PROJECT_LINK.to_string(), meta_font, link_color);
        painter.galley(link_pos, link_galley, link_color);
        if link_resp.hovered() {
            painter.line_segment(
                [
                    pos2(link_rect.left(), link_rect.bottom() + 1.5),
                    pos2(link_rect.right(), link_rect.bottom() + 1.5),
                ],
                Stroke::new(1.0_f32, ACCENT),
            );
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        if link_resp.clicked() {
            ctx.open_url(egui::OpenUrl::new_tab(PROJECT_URL));
        }

        // 中英文切换开关
        painter.rect_filled(toggle_rect, CornerRadius::same(11), Color32::WHITE);
        painter.rect_stroke(
            toggle_rect,
            CornerRadius::same(11),
            Stroke::new(1.0_f32, BORDER_SOFT),
            egui::StrokeKind::Inside,
        );
        let half_w = toggle_w * 0.5;
        let current_lang = i18n::lang();
        for (lang, offset, label) in [(Lang::En, 0.0, "EN"), (Lang::Zh, half_w, "中")] {
            let half_rect = Rect::from_min_size(
                pos2(toggle_rect.left() + offset, toggle_rect.top()),
                vec2(half_w, toggle_h),
            );
            let resp = ui.interact(
                half_rect,
                Id::new(("lang_toggle", lang.code())),
                Sense::click(),
            );
            let active = current_lang == lang;
            if active {
                painter.rect_filled(half_rect, CornerRadius::same(11), ACCENT);
            } else if resp.hovered() {
                painter.rect_filled(half_rect, CornerRadius::same(11), ACCENT_SOFT);
            }
            painter.text(
                half_rect.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(11.0),
                if active { Color32::WHITE } else { MUTED },
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() && !active {
                i18n::set_lang(lang);
                i18n::persist(lang);
                ctx.request_repaint();
            }
        }

        if self.window_button(ui, min_rect, WindowButton::Minimize) {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }
        if self.window_button(ui, max_rect, WindowButton::Maximize) {
            let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        }
        if self.window_button(ui, close_rect, WindowButton::Close) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn window_button(&self, ui: &mut Ui, rect: Rect, kind: WindowButton) -> bool {
        let resp = ui.interact(rect, Id::new(("win_btn", kind as u8)), Sense::click());
        let hovered = resp.hovered();
        let painter = ui.painter();

        if hovered {
            painter.rect_filled(
                rect,
                CornerRadius::ZERO,
                if kind == WindowButton::Close {
                    Color32::from_rgb(0xe8, 0x3b, 0x3b)
                } else {
                    Color32::from_rgb(0xe2, 0xec, 0xf9)
                },
            );
        }

        let color = if hovered && kind == WindowButton::Close {
            Color32::WHITE
        } else {
            Color32::from_rgb(0x44, 0x55, 0x66)
        };
        let c = rect.center();
        let stroke = Stroke::new(1.3_f32, color);
        match kind {
            WindowButton::Minimize => {
                painter.line_segment(
                    [pos2(c.x - 5.0, c.y + 3.0), pos2(c.x + 5.0, c.y + 3.0)],
                    stroke,
                );
            }
            WindowButton::Maximize => {
                painter.rect_stroke(
                    Rect::from_center_size(c, vec2(11.0, 9.0)),
                    CornerRadius::ZERO,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
            WindowButton::Close => {
                painter.line_segment(
                    [pos2(c.x - 5.0, c.y - 5.0), pos2(c.x + 5.0, c.y + 5.0)],
                    stroke,
                );
                painter.line_segment(
                    [pos2(c.x + 5.0, c.y - 5.0), pos2(c.x - 5.0, c.y + 5.0)],
                    stroke,
                );
            }
        }
        resp.clicked()
    }

    // -----------------------------------------------------------------------
    // 左侧面板
    // -----------------------------------------------------------------------

    fn left_panel(&mut self, ui: &mut Ui) {
        ScrollArea::vertical()
            .id_salt("left_panel")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                theme::group_box(ui, t("连接设置"), |ui| self.connection_group(ui));
                ui.add_space(6.0);
                theme::group_box(ui, t("芯片选择"), |ui| self.chip_group(ui));
                ui.add_space(6.0);
                theme::group_box(ui, t("烧录操作"), |ui| self.operation_group(ui));
            });
    }

    fn labeled_row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                vec2(44.0, 24.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.label(RichText::new(label).size(12.5).color(LABEL));
                },
            );
            add(ui);
        });
    }

    fn connection_group(&mut self, ui: &mut Ui) {
        let old_probe = self.probe.clone();
        let old_protocol = self.protocol;
        let old_speed = self.speed_index;
        let old_reset = self.reset_index;

        Self::labeled_row(ui, t("设备:"), |ui| {
            let text = match &self.probe {
                Some(probe) => probe.label(),
                None if self.probes.is_empty() => tt!("未检测到调试器"),
                None => tt!("选择调试器"),
            };
            let width = ui.available_width();
            egui::ComboBox::from_id_salt("probe_combo")
                .width(width)
                .selected_text(RichText::new(text).size(12.0))
                .show_ui(ui, |ui| {
                    if self.probes.is_empty() {
                        ui.label(RichText::new(t("未检测到调试器")).size(12.0).color(MUTED));
                    }
                    let probes = self.probes.clone();
                    for probe in &probes {
                        let selected = self.probe.as_ref() == Some(probe);
                        if ui
                            .selectable_label(selected, RichText::new(probe.label()).size(12.0))
                            .clicked()
                        {
                            self.probe = Some(probe.clone());
                        }
                    }
                });
        });

        Self::labeled_row(ui, t("端口:"), |ui| {
            let width = ui.available_width();
            egui::ComboBox::from_id_salt("protocol_combo")
                .width(width)
                .selected_text(RichText::new(self.protocol.label()).size(12.0))
                .show_ui(ui, |ui| {
                    for protocol in [Protocol::Swd, Protocol::Jtag] {
                        if ui
                            .selectable_value(
                                &mut self.protocol,
                                protocol,
                                RichText::new(protocol.label()).size(12.0),
                            )
                            .clicked()
                        {}
                    }
                });
        });

        Self::labeled_row(ui, t("速率:"), |ui| {
            let width = ui.available_width();
            egui::ComboBox::from_id_salt("speed_combo")
                .width(width)
                .selected_text(
                    RichText::new(SPEEDS[self.speed_index.min(SPEEDS.len() - 1)].0).size(12.0),
                )
                .show_ui(ui, |ui| {
                    for (index, (label, _)) in SPEEDS.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.speed_index,
                            index,
                            RichText::new(t(*label)).size(12.0),
                        );
                    }
                });
        });

        Self::labeled_row(ui, t("复位:"), |ui| {
            let width = ui.available_width();
            egui::ComboBox::from_id_salt("reset_combo")
                .width(width)
                .selected_text(
                    RichText::new(t(RESET_MODES[self.reset_index.min(RESET_MODES.len() - 1)].0))
                        .size(12.0),
                )
                .show_ui(ui, |ui| {
                    for (index, (label, _)) in RESET_MODES.iter().enumerate() {
                        ui.selectable_value(
                            &mut self.reset_index,
                            index,
                            RichText::new(t(*label)).size(12.0),
                        );
                    }
                });
        });

        if old_probe != self.probe
            || old_protocol != self.protocol
            || old_speed != self.speed_index
            || old_reset != self.reset_index
        {
            self.reconnect();
        }
    }

    fn chip_group(&mut self, ui: &mut Ui) {
        let Some(db) = self.chips.as_ref() else {
            ui.label(RichText::new(t("芯片库加载中…")).size(12.0).color(MUTED));
            return;
        };

        let vendor_names = db.vendor_names();
        let families = db.family_names(self.vendor);
        let chips = db.chip_names(self.vendor, self.family);

        let query = self.search.trim().to_string();
        let search_results = if query.is_empty() {
            Vec::new()
        } else {
            db.search(&query, 300)
        };
        let result_labels: Vec<String> = search_results
            .iter()
            .map(|(v, f, c)| {
                let family = &db.vendors[*v].families[*f];
                format!("{} · {}", family.name, family.chips[*c])
            })
            .collect();

        let old_selection = (self.vendor, self.family, self.chip);

        Self::labeled_row(ui, t("厂家:"), |ui| {
            let width = ui.available_width();
            let current = vendor_names.get(self.vendor).cloned().unwrap_or_default();
            egui::ComboBox::from_id_salt("vendor_combo")
                .width(width)
                .selected_text(RichText::new(current).size(12.0))
                .show_ui(ui, |ui| {
                    for (index, name) in vendor_names.iter().enumerate() {
                        if ui
                            .selectable_value(
                                &mut self.vendor,
                                index,
                                RichText::new(name).size(12.0),
                            )
                            .clicked()
                        {
                            self.family = 0;
                            self.chip = 0;
                        }
                    }
                });
        });

        Self::labeled_row(ui, t("系列:"), |ui| {
            let width = ui.available_width();
            let current = families.get(self.family).cloned().unwrap_or_default();
            egui::ComboBox::from_id_salt("family_combo")
                .width(width)
                .selected_text(RichText::new(current).size(12.0))
                .show_ui(ui, |ui| {
                    for (index, name) in families.iter().enumerate() {
                        if ui
                            .selectable_value(
                                &mut self.family,
                                index,
                                RichText::new(name).size(12.0),
                            )
                            .clicked()
                        {
                            self.chip = 0;
                        }
                    }
                });
        });

        Self::labeled_row(ui, t("型号:"), |ui| {
            let width = ui.available_width();
            let current = if search_results.is_empty() {
                chips.get(self.chip).cloned().unwrap_or_default()
            } else {
                result_labels
                    .first()
                    .cloned()
                    .unwrap_or_else(|| tt!("无匹配型号"))
            };

            egui::ComboBox::from_id_salt("chip_combo")
                .width(width)
                .selected_text(RichText::new(current).size(12.0))
                .show_ui(ui, |ui| {
                    if search_results.is_empty() {
                        for (index, name) in chips.iter().enumerate() {
                            ui.selectable_value(
                                &mut self.chip,
                                index,
                                RichText::new(name).size(12.0),
                            );
                        }
                    } else {
                        for (index, label) in result_labels.iter().enumerate() {
                            let selected = search_results
                                .get(index)
                                .map(|(v, f, c)| {
                                    (*v, *f, *c) == (self.vendor, self.family, self.chip)
                                })
                                .unwrap_or(false);
                            if ui
                                .selectable_label(selected, RichText::new(label).size(12.0))
                                .clicked()
                                && let Some((v, f, c)) = search_results.get(index)
                            {
                                self.vendor = *v;
                                self.family = *f;
                                self.chip = *c;
                            }
                        }
                    }
                });
        });

        ui.horizontal(|ui| {
            let text_width = (ui.available_width() - 30.0).max(80.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(RichText::new(t("输入型号关键字，如 STM32")).size(11.5))
                    .desired_width(text_width),
            );
            let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
            let color = if resp.hovered() { ACCENT } else { MUTED };
            let center = pos2(rect.center().x - 1.0, rect.center().y - 1.0);
            ui.painter()
                .circle_stroke(center, 5.0, Stroke::new(1.6_f32, color));
            ui.painter().line_segment(
                [
                    pos2(center.x + 3.5, center.y + 3.5),
                    pos2(center.x + 7.0, center.y + 7.0),
                ],
                Stroke::new(1.8_f32, color),
            );
        });

        ui.add_space(3.0);
        ui.horizontal(|ui| {
            let busy = self.busy.is_some();
            let spacing = ui.spacing().item_spacing.x;
            let width = ((ui.available_width() - spacing) * 0.5).max(80.0);
            if theme::op_button_sized(ui, width, 28.0, theme::Icon::Plus, t("导入芯片…"), !busy)
                .clicked()
            {
                self.open_picker(PickerMode::OpenTarget);
            }
            if theme::op_button_sized(ui, width, 28.0, theme::Icon::Gear, t("管理自定义"), true)
                .clicked()
            {
                self.custom_dialog_open = true;
                self.custom_delete_confirm = None;
            }
        });

        if old_selection != (self.vendor, self.family, self.chip) && self.connected_chip.is_some() {
            self.push_log(LogLevel::Warn, tt!("芯片型号已更改，重新连接后生效"));
        }
    }

    fn operation_group(&mut self, ui: &mut Ui) {
        let busy = self.busy.is_some();
        let connected = self.connected_chip.is_some();
        let has_firmware = self.firmware.is_some();

        if theme::op_button(
            ui,
            36.0,
            theme::Icon::Download,
            t("烧录"),
            !busy && connected && has_firmware,
        )
        .clicked()
        {
            self.request_flash();
        }
        ui.add_space(4.0);
        if theme::op_button(
            ui,
            36.0,
            theme::Icon::Check,
            t("校验"),
            !busy && connected && has_firmware,
        )
        .clicked()
        {
            self.request_verify();
        }
        ui.add_space(4.0);
        if theme::op_button(ui, 34.0, theme::Icon::Erase, t("擦除"), !busy && connected).clicked()
        {
            self.push_log(
                LogLevel::Info,
                tt!("***************** 擦除 *****************"),
            );
            self.pending.push(Command::Erase);
        }
        ui.add_space(4.0);
        if theme::op_button(ui, 34.0, theme::Icon::Read, t("读取"), !busy && connected).clicked()
        {
            self.push_log(
                LogLevel::Info,
                tt!("***************** 读取 *****************"),
            );
            self.pending.push(Command::ReadFlash);
        }
        ui.add_space(4.0);
        if theme::op_button(ui, 34.0, theme::Icon::Reset, t("复位"), !busy && connected).clicked()
        {
            self.pending.push(Command::Reset);
        }

        ui.add_space(5.0);
        ui.horizontal(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            let width = ((ui.available_width() - spacing) * 0.5).max(90.0);
            if theme::op_button_sized(
                ui,
                width,
                31.0,
                theme::Icon::Memory,
                t("读写内存"),
                !busy && connected,
            )
            .clicked()
            {
                self.mem.open = true;
            }
            if theme::op_button_sized(ui, width, 31.0, theme::Icon::Stop, t("终止"), busy).clicked()
            {
                self.worker.request_abort();
                self.push_log(LogLevel::Warn, tt!("已请求终止当前操作"));
            }
        });

        ui.add_space(8.0);
        ui.checkbox(
            &mut self.opt_chip_erase,
            RichText::new(t("整片擦除")).size(12.0),
        );
        ui.checkbox(
            &mut self.opt_auto_flash,
            RichText::new(t("自动烧录")).size(12.0),
        );
        ui.checkbox(
            &mut self.opt_auto_connect,
            RichText::new(t("自动连接")).size(12.0),
        );
    }

    // -----------------------------------------------------------------------
    // 右侧面板
    // -----------------------------------------------------------------------

    fn right_panel(&mut self, ui: &mut Ui) {
        let total_height = ui.available_height();
        let firmware_height = (total_height * 0.50).clamp(190.0, total_height - 150.0);

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), firmware_height),
            Layout::top_down(Align::Min),
            |ui| {
                theme::group_box(ui, t("固件"), |ui| self.firmware_group(ui));
            },
        );

        ui.add_space(6.0);

        let rest = ui.available_height().max(120.0);
        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), rest),
            Layout::top_down(Align::Min),
            |ui| {
                theme::group_box(ui, t("日志"), |ui| self.log_group(ui));
            },
        );
    }

    fn firmware_group(&mut self, ui: &mut Ui) {
        let full = ui.available_size();
        ui.set_min_size(full);

        if self.view.is_none() {
            self.drop_zone(ui, full);
            return;
        }

        self.hex_view(ui, full);
    }

    fn drop_zone(&mut self, ui: &mut Ui, size: egui::Vec2) {
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let hovering = !ui.input(|i| i.raw.hovered_files.is_empty());

        let painter = ui.painter();
        painter.rect_filled(
            rect,
            CornerRadius::same(4),
            if hovering { DROP_FILL_HOT } else { DROP_FILL },
        );
        theme::dashed_rect(
            painter,
            rect.shrink(3.0),
            if hovering {
                DROP_BORDER_HOT
            } else {
                DROP_BORDER
            },
            6.0,
            5.0,
        );

        let cx = rect.center().x;
        let cy = rect.center().y;

        theme::draw_folder(painter, pos2(cx, cy - 44.0), 66.0, true);

        painter.text(
            pos2(cx, cy + 22.0),
            Align2::CENTER_CENTER,
            t("拖拽 .bin / .hex 文件到此处"),
            FontId::proportional(14.5),
            ACCENT_SOFT,
        );
        painter.text(
            pos2(cx, cy + 46.0),
            Align2::CENTER_CENTER,
            t("支持固件文件快速加载"),
            FontId::proportional(12.5),
            MUTED,
        );
        painter.text(
            pos2(cx, cy + 68.0),
            Align2::CENTER_CENTER,
            t("点击此处选择文件"),
            FontId::proportional(11.0),
            Color32::from_rgb(0x93, 0xac, 0xc6),
        );

        if response.clicked() {
            self.open_picker(PickerMode::OpenFirmware);
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    fn hex_view(&mut self, ui: &mut Ui, full: egui::Vec2) {
        const ROW_HEIGHT: f32 = 15.0;
        const BYTES_PER_ROW: usize = 16;
        const HEADER_HEIGHT: f32 = 18.0;
        const FOOTER_HEIGHT: f32 = 26.0;

        let Some(view) = &self.view else { return };

        let font = FontId::monospace(11.5);
        let char_w = ui.fonts(|f| f.glyph_width(&font, '0'));

        let addr_x = 4.0;
        let bytes_x = addr_x + 9.0 * char_w + 8.0;
        let ascii_x = bytes_x + (BYTES_PER_ROW as f32) * 3.0 * char_w + 8.0;

        // 表头
        let (header_rect, _) =
            ui.allocate_exact_size(vec2(ui.available_width(), HEADER_HEIGHT), Sense::hover());
        let painter = ui.painter();
        let header_font = FontId::monospace(11.0);
        painter.text(
            pos2(header_rect.left() + addr_x, header_rect.center().y),
            Align2::LEFT_CENTER,
            "ADDRESS",
            header_font.clone(),
            MUTED,
        );
        for i in 0..BYTES_PER_ROW {
            painter.text(
                pos2(
                    header_rect.left() + bytes_x + i as f32 * 3.0 * char_w,
                    header_rect.center().y,
                ),
                Align2::LEFT_CENTER,
                format!("{i:02X}"),
                header_font.clone(),
                MUTED,
            );
        }
        painter.text(
            pos2(header_rect.left() + ascii_x, header_rect.center().y),
            Align2::LEFT_CENTER,
            "0123456789ABCDEF",
            header_font,
            MUTED,
        );

        let scroll_height = (full.y - HEADER_HEIGHT - FOOTER_HEIGHT - 30.0).max(40.0);

        let view_base = view.base;
        let rows = view.data.len().div_ceil(BYTES_PER_ROW);
        let data = &view.data;

        ui.allocate_ui_with_layout(
            vec2(ui.available_width(), scroll_height),
            Layout::top_down(Align::Min),
            |ui| {
                ScrollArea::vertical()
                    .id_salt("hex_scroll")
                    .auto_shrink([false, false])
                    .show_rows(ui, ROW_HEIGHT, rows, |ui, range| {
                        let text_font = FontId::monospace(11.5);
                        for row in range {
                            let (row_rect, row_resp) = ui.allocate_exact_size(
                                vec2(ui.available_width(), ROW_HEIGHT),
                                Sense::hover(),
                            );
                            if row_resp.hovered() {
                                ui.painter().rect_filled(
                                    row_rect,
                                    CornerRadius::ZERO,
                                    Color32::from_rgb(0xf2, 0xf7, 0xfd),
                                );
                            }
                            let offset = row * BYTES_PER_ROW;
                            let end = (offset + BYTES_PER_ROW).min(data.len());
                            let chunk = &data[offset.min(data.len())..end];

                            ui.painter().text(
                                pos2(row_rect.left() + addr_x, row_rect.center().y),
                                Align2::LEFT_CENTER,
                                format!("{:08X}", view_base + offset as u64),
                                text_font.clone(),
                                HEX_ADDRESS,
                            );

                            let hex: String = (0..BYTES_PER_ROW)
                                .map(|i| match chunk.get(i) {
                                    Some(b) => format!("{b:02X} "),
                                    None => "   ".to_string(),
                                })
                                .collect();
                            ui.painter().text(
                                pos2(row_rect.left() + bytes_x, row_rect.center().y),
                                Align2::LEFT_CENTER,
                                hex,
                                text_font.clone(),
                                HEX_BYTE,
                            );

                            let ascii: String = chunk
                                .iter()
                                .map(|b| {
                                    if (0x20..0x7F).contains(b) {
                                        *b as char
                                    } else {
                                        '.'
                                    }
                                })
                                .collect();
                            ui.painter().text(
                                pos2(row_rect.left() + ascii_x, row_rect.center().y),
                                Align2::LEFT_CENTER,
                                ascii,
                                text_font.clone(),
                                HEX_ASCII,
                            );
                        }
                    });
            },
        );

        // 底部信息栏
        let source = view.source;
        let label = view.label.clone();
        let size = format_bytes(view.data.len() as u64);
        let crc = view.crc32;
        let base = view.base;

        ui.horizontal(|ui| {
            let prefix = match source {
                HexSource::Firmware => t("固件"),
                HexSource::Target => t("目标"),
            };
            ui.label(
                RichText::new(tt!(
                    "{}: {} · {} · 起始 0x{} · CRC32 {}",
                    prefix,
                    label,
                    size,
                    format!("{base:08X}"),
                    format!("{crc:08X}")
                ))
                .size(11.5)
                .color(MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::small_button(ui, t("清除")).clicked() {
                    self.view = None;
                    self.firmware = None;
                    self.push_log(LogLevel::Info, tt!("已清除固件视图"));
                }
                if theme::small_button(ui, t("导出")).clicked() {
                    self.open_picker(PickerMode::SaveDump);
                }
            });
        });
    }

    fn log_group(&mut self, ui: &mut Ui) {
        let full = ui.available_size();
        ui.set_min_size(full);

        if let Some(kind) = self.busy {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(tt!("{}中…", kind.label()))
                        .size(12.0)
                        .color(ACCENT),
                );
                ui.label(RichText::new(&self.progress_text).size(11.5).color(MUTED));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.add(
                        egui::ProgressBar::new(self.progress)
                            .desired_width(200.0)
                            .text(
                                RichText::new(format!("{:.0}%", self.progress * 100.0)).size(11.0),
                            ),
                    );
                });
            });
            ui.add_space(2.0);
        }

        let height = ui.available_height();
        ScrollArea::vertical()
            .id_salt("log_scroll")
            .auto_shrink([false, false])
            .max_height(height)
            .stick_to_bottom(self.follow_log)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                for line in &self.logs {
                    let color = match line.level {
                        LogLevel::Info => Color32::from_rgb(0x36, 0x45, 0x56),
                        LogLevel::Ok => GREEN,
                        LogLevel::Warn => Color32::from_rgb(0xb3, 0x7a, 0x12),
                        LogLevel::Error => RED,
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        ui.label(
                            RichText::new(format!("[{}]", line.time))
                                .monospace()
                                .size(11.0)
                                .color(Color32::from_rgb(0x86, 0x9a, 0xb0)),
                        );
                        ui.label(RichText::new(&line.message).size(12.0).color(color));
                    });
                }
            });
    }

    // -----------------------------------------------------------------------
    // 状态栏
    // -----------------------------------------------------------------------

    fn frame_status_bar(&mut self, ui: &mut Ui, rect: Rect) {
        let painter = ui.painter();
        painter.rect_filled(
            rect,
            CornerRadius::ZERO,
            Color32::from_rgb(0xf4, 0xf9, 0xff),
        );
        painter.rect_filled(
            Rect::from_min_size(rect.min, vec2(rect.width(), 1.0)),
            CornerRadius::ZERO,
            BORDER_SOFT,
        );

        let (dot, text, text_color) = match &self.connected_chip {
            Some(chip) => (GREEN, tt!("目标 {} 已连接", chip), LABEL),
            None if self.busy == Some(OpKind::Connect) => (ORANGE, tt!("正在连接目标…"), LABEL),
            None => (
                Color32::from_rgb(0x9a, 0xa8, 0xb7),
                tt!("未连接目标"),
                MUTED,
            ),
        };

        painter.circle_filled(pos2(rect.left() + 12.0, rect.center().y), 4.0, dot);
        painter.text(
            pos2(rect.left() + 22.0, rect.center().y),
            Align2::LEFT_CENTER,
            text,
            FontId::proportional(12.0),
            text_color,
        );

        if let Some(probe) = &self.connected_probe {
            painter.text(
                pos2(rect.center().x, rect.center().y),
                Align2::CENTER_CENTER,
                probe,
                FontId::proportional(11.5),
                MUTED,
            );
        }

        painter.text(
            pos2(rect.right() - 12.0, rect.center().y),
            Align2::RIGHT_CENTER,
            &self.status_message,
            FontId::proportional(12.0),
            if self.status_ok { MUTED } else { RED },
        );
    }

    fn first_custom_selection(&self) -> Option<(usize, usize, usize)> {
        let db = self.chips.as_ref()?;
        let vendor = db
            .vendors
            .iter()
            .position(|vendor| vendor.name == CUSTOM_VENDOR)?;
        if db.vendors[vendor].families.is_empty() {
            return None;
        }
        Some((vendor, 0, 0))
    }

    fn open_targets_dir(&mut self) {
        let dir = match custom::ensure_targets_dir() {
            Ok(dir) => dir,
            Err(e) => {
                self.push_log(LogLevel::Error, e);
                return;
            }
        };

        #[cfg(target_os = "windows")]
        let result = std::process::Command::new("explorer").arg(&dir).spawn();
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("open").arg(&dir).spawn();
        #[cfg(all(unix, not(target_os = "macos")))]
        let result = std::process::Command::new("xdg-open").arg(&dir).spawn();

        #[allow(unused)]
        if let Err(e) = result {
            self.push_log(LogLevel::Warn, tt!("无法打开目录 {}: {}", dir.display(), e));
        }
    }

    // -----------------------------------------------------------------------
    // 自定义芯片管理窗口
    // -----------------------------------------------------------------------

    fn show_custom_dialog(&mut self, ctx: &egui::Context) {
        if !self.custom_dialog_open {
            return;
        }

        let mut open = self.custom_dialog_open;
        let mut close_requested = false;
        let mut import_requested = false;
        let mut delete_requested: Option<String> = None;
        let targets = self.custom_targets.clone();
        let targets_dir = custom::targets_dir().display().to_string();
        let busy = self.busy.is_some();

        egui::Window::new(t("自定义芯片"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(720.0)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(t(
                        "支持 CMSIS-Pack（.pack / 解压目录 / .pdsc）与 probe-rs 目标描述 YAML，由 target-gen 生成并保存到本地配置目录。",
                    ))
                    .size(11.5)
                    .color(MUTED),
                );
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("存放目录:")).size(11.5).color(MUTED));
                    ui.label(
                        RichText::new(&targets_dir)
                            .monospace()
                            .size(11.0)
                            .color(LABEL),
                    );
                    if theme::small_button(ui, t("打开目录")).clicked() {
                        self.open_targets_dir();
                    }
                });

                ui.add_space(4.0);
                theme::hline(ui, BORDER_SOFT);
                ui.add_space(4.0);

                if targets.is_empty() {
                    ui.label(
                        RichText::new(t("还没有导入自定义芯片，点击下方「导入芯片…」开始。"))
                            .size(12.0)
                            .color(MUTED),
                    );
                } else {
                    ScrollArea::vertical()
                        .id_salt("custom_targets")
                        .max_height(260.0)
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            for target in &targets {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&target.name).size(12.5).color(LABEL));
                                    ui.label(
                                        RichText::new(tt!("{} 个型号", target.chips))
                                            .size(11.0)
                                            .color(MUTED),
                                    );
                                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                        let confirming = self.custom_delete_confirm.as_deref()
                                            == Some(target.name.as_str());
                                        let label = if confirming { t("确认删除") } else { t("删除") };
                                        if theme::small_button(ui, label).clicked() {
                                            if confirming {
                                                delete_requested = Some(target.name.clone());
                                            } else {
                                                self.custom_delete_confirm =
                                                    Some(target.name.clone());
                                            }
                                        }
                                        if confirming
                                            && theme::small_button(ui, t("取消")).clicked()
                                        {
                                            self.custom_delete_confirm = None;
                                        }
                                    });
                                });
                                ui.label(
                                    RichText::new(target.path.display().to_string())
                                        .monospace()
                                        .size(10.0)
                                        .color(Color32::from_rgb(0x8a, 0x9a, 0xac)),
                                );
                                ui.add_space(2.0);
                            }
                        });
                }

                ui.add_space(6.0);
                theme::hline(ui, BORDER_SOFT);
                ui.horizontal(|ui| {
                    if theme::small_button(ui, t("导入芯片…")).clicked() {
                        import_requested = true;
                    }
                    if busy {
                        ui.label(RichText::new(t("正在处理，请稍候…")).size(11.0).color(ACCENT));
                    } else {
                        ui.label(
                            RichText::new(t("提示：导入 .pdsc 时会扫描其所在目录以获取 flash 算法文件"))
                                .size(10.5)
                                .color(MUTED),
                        );
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::small_button(ui, t("关闭")).clicked() {
                            close_requested = true;
                        }
                    });
                });
            });

        if close_requested {
            open = false;
        }
        if import_requested {
            self.open_picker(PickerMode::OpenTarget);
            open = false;
        }
        if let Some(name) = delete_requested {
            self.custom_delete_confirm = None;
            self.pending.push(Command::RemoveTarget { name });
        }
        self.custom_dialog_open = open;
    }

    // -----------------------------------------------------------------------
    // 文件选择器
    // -----------------------------------------------------------------------

    fn open_picker(&mut self, mode: PickerMode) {
        let start_dir = match mode {
            PickerMode::OpenFirmware => self
                .firmware
                .as_ref()
                .and_then(|f| f.path.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(home_dir),
            PickerMode::SaveDump => self
                .firmware
                .as_ref()
                .and_then(|f| f.path.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(home_dir),
            PickerMode::OpenTarget => std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_dir())
                .unwrap_or_else(home_dir),
        };

        let file_name = match mode {
            PickerMode::OpenFirmware => self
                .firmware
                .as_ref()
                .map(|f| f.name.clone())
                .unwrap_or_default(),
            PickerMode::SaveDump => {
                let base = self
                    .view
                    .as_ref()
                    .map(|v| v.base)
                    .unwrap_or(self.flash_base);
                format!("flash_{base:08X}.bin")
            }
            PickerMode::OpenTarget => String::new(),
        };

        let mut picker = FilePicker {
            mode,
            dir: start_dir,
            entries: Vec::new(),
            file_name,
            path_input: String::new(),
            error: None,
            show_all: false,
        };
        picker.path_input = picker.dir.display().to_string();
        refresh_picker(&mut picker);
        self.picker = Some(picker);
    }

    fn show_picker(&mut self, ctx: &egui::Context) {
        let Some(mut picker) = self.picker.take() else {
            return;
        };
        let mut open = true;
        let mut close_requested = false;
        let mut action: Option<(PickerMode, PathBuf)> = None;
        let mut navigate_to: Option<PathBuf> = None;

        egui::Window::new(match picker.mode {
            PickerMode::OpenFirmware => t("打开固件文件"),
            PickerMode::SaveDump => t("导出为文件"),
            PickerMode::OpenTarget => t("导入自定义芯片"),
        })
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_width(620.0)
        .default_height(460.0)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("路径:")).size(12.0).color(LABEL));
                let width = (ui.available_width() - 70.0).max(120.0);
                ui.add(
                    egui::TextEdit::singleline(&mut picker.path_input)
                        .desired_width(width)
                        .font(FontId::monospace(12.0)),
                );
                if theme::small_button(ui, t("转到")).clicked() {
                    let path = PathBuf::from(picker.path_input.trim());
                    if path.is_dir() {
                        picker.dir = path;
                        refresh_picker(&mut picker);
                    } else if path.is_file() {
                        action = Some((picker.mode, path));
                    } else {
                        picker.error = Some(tt!("路径不存在"));
                    }
                }
            });

            ui.add_space(2.0);

            ScrollArea::vertical()
                .id_salt("picker_list")
                .auto_shrink([false, false])
                .max_height(280.0)
                .show(ui, |ui| {
                    for entry in &picker.entries {
                        let label = if entry.is_dir {
                            tt!("[目录] {}", entry.name)
                        } else {
                            entry.name.clone()
                        };
                        let selected = !entry.is_dir && picker.file_name == entry.name;
                        let resp = ui.selectable_label(selected, RichText::new(label).size(12.0));
                        if resp.clicked() {
                            if entry.is_dir {
                                navigate_to = Some(entry.path.clone());
                            } else {
                                picker.file_name = entry.name.clone();
                            }
                        }
                        if resp.double_clicked() && !entry.is_dir {
                            action = Some((picker.mode, entry.path.clone()));
                        }
                    }
                });

            if let Some(dir) = navigate_to.take() {
                picker.dir = dir;
                refresh_picker(&mut picker);
            }

            ui.add_space(4.0);
            theme::hline(ui, BORDER_SOFT);
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("文件名:")).size(12.0).color(LABEL));
                let width = (ui.available_width() - 130.0).max(120.0);
                ui.add(
                    egui::TextEdit::singleline(&mut picker.file_name)
                        .desired_width(width)
                        .font(FontId::monospace(12.0)),
                );
                let confirm = match picker.mode {
                    PickerMode::OpenFirmware => t("打开"),
                    PickerMode::SaveDump => t("保存"),
                    PickerMode::OpenTarget => t("导入"),
                };
                if theme::small_button(ui, confirm).clicked() {
                    let path = picker.dir.join(&picker.file_name);
                    action = Some((picker.mode, path));
                }
                if theme::small_button(ui, t("取消")).clicked() {
                    close_requested = true;
                }
            });

            if let Some(error) = &picker.error {
                ui.label(RichText::new(error).size(11.5).color(RED));
            }
        });

        if close_requested {
            open = false;
        }

        if let Some((mode, path)) = action {
            match mode {
                PickerMode::OpenFirmware => {
                    self.load_firmware_file(path);
                    open = false;
                }
                PickerMode::OpenTarget => {
                    self.select_custom_after_import = true;
                    self.pending.push(Command::ImportTarget { path });
                    open = false;
                }
                PickerMode::SaveDump => {
                    if let Some(view) = &self.view {
                        match std::fs::write(&path, &view.data) {
                            Ok(()) => {
                                self.push_log(
                                    LogLevel::Ok,
                                    tt!(
                                        "已导出 {} ({})",
                                        path.display(),
                                        format_bytes(view.data.len() as u64)
                                    ),
                                );
                                self.status_ok = true;
                                self.status_message = tt!("导出完成");
                                open = false;
                            }
                            Err(e) => {
                                self.push_log(LogLevel::Error, tt!("写入文件失败: {}", e));
                            }
                        }
                    } else {
                        open = false;
                    }
                }
            }
        }

        if open {
            self.picker = Some(picker);
        }
    }

    // -----------------------------------------------------------------------
    // 读写内存窗口
    // -----------------------------------------------------------------------

    fn show_mem_dialog(&mut self, ctx: &egui::Context) {
        if !self.mem.open {
            return;
        }

        let mut open = self.mem.open;
        let mut read_request: Option<(u64, usize)> = None;
        let mut write_request: Option<(u64, Vec<u8>)> = None;

        egui::Window::new(t("读写内存"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_width(600.0)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("地址")).size(12.0).color(LABEL));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.mem.address)
                            .desired_width(120.0)
                            .font(FontId::monospace(12.0)),
                    );
                    ui.label(RichText::new(t("长度")).size(12.0).color(LABEL));
                    ui.add(
                        egui::TextEdit::singleline(&mut self.mem.length)
                            .desired_width(80.0)
                            .font(FontId::monospace(12.0)),
                    );
                    if ui
                        .add_enabled(
                            self.busy.is_none() && self.connected_chip.is_some(),
                            egui::Button::new(RichText::new(t("读取")).size(12.0)),
                        )
                        .clicked()
                    {
                        match firmware::parse_address(&self.mem.address) {
                            Ok(address) => {
                                let len = self.mem.length.trim().parse::<usize>().or_else(|_| {
                                    u64::from_str_radix(self.mem.length.trim(), 16)
                                        .map(|v| v as usize)
                                });
                                match len {
                                    Ok(len) if len > 0 => {
                                        self.mem.error = None;
                                        self.mem.status = Some(tt!("正在读取…"));
                                        read_request = Some((address, len));
                                    }
                                    _ => {
                                        self.mem.error = Some(tt!("长度必须为正整数"));
                                    }
                                }
                            }
                            Err(e) => self.mem.error = Some(e),
                        }
                    }
                });

                ui.add_space(4.0);
                ScrollArea::vertical()
                    .id_salt("mem_read_scroll")
                    .max_height(160.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.mem.read_result.is_empty() {
                            ui.label(
                                RichText::new(t("读取结果将显示在这里"))
                                    .size(11.5)
                                    .color(MUTED),
                            );
                        } else {
                            ui.label(
                                RichText::new(&self.mem.read_result)
                                    .monospace()
                                    .size(11.0)
                                    .color(HEX_BYTE),
                            );
                        }
                    });

                ui.add_space(4.0);
                theme::hline(ui, BORDER_SOFT);
                ui.add_space(4.0);
                ui.label(
                    RichText::new(t("写入数据（十六进制，可用空格分隔）"))
                        .size(11.5)
                        .color(MUTED),
                );
                ui.add(
                    egui::TextEdit::multiline(&mut self.mem.write_data)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .font(FontId::monospace(12.0))
                        .hint_text(t("例如: 01 02 03 04 …")),
                );

                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.busy.is_none() && self.connected_chip.is_some(),
                            egui::Button::new(RichText::new(t("写入")).size(12.0)),
                        )
                        .clicked()
                    {
                        match (
                            firmware::parse_address(&self.mem.address),
                            firmware::parse_hex_bytes(&self.mem.write_data),
                        ) {
                            (Ok(address), Ok(data)) if !data.is_empty() => {
                                self.mem.error = None;
                                self.mem.status = Some(tt!("正在写入 {} 字节…", data.len()));
                                write_request = Some((address, data));
                            }
                            (Err(e), _) => self.mem.error = Some(e),
                            (_, Err(e)) => self.mem.error = Some(e),
                            _ => self.mem.error = Some(tt!("没有可写入的数据")),
                        }
                    }
                    if let Some(status) = &self.mem.status {
                        ui.label(RichText::new(status).size(11.5).color(MUTED));
                    }
                    if let Some(error) = &self.mem.error {
                        ui.label(RichText::new(error).size(11.5).color(RED));
                    }
                });
            });

        self.mem.open = open;

        if let Some((address, len)) = read_request {
            self.pending.push(Command::ReadMemory { address, len });
        }
        if let Some((address, data)) = write_request {
            self.pending.push(Command::WriteMemory { address, data });
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WindowButton {
    Minimize,
    Maximize,
    Close,
}

// ---------------------------------------------------------------------------
// eframe
// ---------------------------------------------------------------------------

impl eframe::App for FlasherApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events();
        self.tick();
        self.handle_dropped_files(ctx);

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.picker = None;
            self.mem.open = false;
            self.custom_dialog_open = false;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::O)) {
            self.open_picker(PickerMode::OpenFirmware);
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(egui::Margin::same(0)),
            )
            .show(ctx, |ui| {
                let rect = ui.max_rect();
                let title_h = 38.0;
                let status_h = 26.0;

                let title_rect = Rect::from_min_size(rect.min, vec2(rect.width(), title_h));
                self.frame_title_bar(ui, ctx, title_rect);

                let body_top = rect.top() + title_h;
                let body_bottom = (rect.bottom() - status_h).max(body_top + 200.0);
                let gap = 9.0;
                let left_width = 336.0;

                let left_rect = Rect::from_min_max(
                    pos2(rect.left() + gap, body_top + gap),
                    pos2(rect.left() + gap + left_width, body_bottom - gap),
                );
                let right_rect = Rect::from_min_max(
                    pos2(left_rect.right() + gap, body_top + gap),
                    pos2(rect.right() - gap, body_bottom - gap),
                );

                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(left_rect)
                        .layout(Layout::top_down(Align::Min)),
                    |ui| self.left_panel(ui),
                );
                ui.scope_builder(
                    egui::UiBuilder::new()
                        .max_rect(right_rect)
                        .layout(Layout::top_down(Align::Min)),
                    |ui| self.right_panel(ui),
                );

                let status_rect = Rect::from_min_size(
                    pos2(rect.left(), body_bottom),
                    vec2(rect.width(), rect.bottom() - body_bottom),
                );
                self.frame_status_bar(ui, status_rect);
            });

        self.show_picker(ctx);
        self.show_mem_dialog(ctx);
        self.show_custom_dialog(ctx);
        self.flush_pending();

        self.handle_screenshot(ctx);

        if self.busy.is_some() {
            ctx.request_repaint_after(Duration::from_millis(60));
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}

impl FlasherApp {
    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        if self.shot_path.is_none() {
            return;
        }

        self.shot_frames += 1;
        if self.shot_frames == 30 {
            ctx.send_viewport_cmd(ViewportCommand::Screenshot(egui::UserData::default()));
            ctx.request_repaint();
            return;
        }

        let screenshot = ctx.input(|i| {
            i.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });

        if let Some(image) = screenshot
            && let Some(path) = self.shot_path.take()
        {
            let mut ppm = format!("P6\n{} {}\n255\n", image.size[0], image.size[1]).into_bytes();
            for pixel in &image.pixels {
                ppm.extend_from_slice(&[pixel.r(), pixel.g(), pixel.b()]);
            }
            let _ = std::fs::write(&path, ppm);
        }
    }
}

// ---------------------------------------------------------------------------
// 文件选择器辅助
// ---------------------------------------------------------------------------

fn is_picker_target_file(path: &Path, mode: PickerMode) -> bool {
    match mode {
        PickerMode::OpenFirmware => FirmwareFormat::from_path(path).is_some(),
        PickerMode::SaveDump => true,
        PickerMode::OpenTarget => {
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase());
            matches!(
                ext.as_deref(),
                Some("pack") | Some("zip") | Some("pdsc") | Some("yaml") | Some("yml")
            )
        }
    }
}

fn home_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")))
}

fn refresh_picker(picker: &mut FilePicker) {
    picker.entries.clear();
    picker.path_input = picker.dir.display().to_string();

    // 上一级
    if let Some(parent) = picker.dir.parent() {
        picker.entries.push(PickerEntry {
            name: "..".to_string(),
            path: parent.to_path_buf(),
            is_dir: true,
        });
    }

    let mut dirs = Vec::new();
    let mut files = Vec::new();

    match std::fs::read_dir(&picker.dir) {
        Ok(read_dir) => {
            for entry in read_dir.flatten() {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let is_dir = path.is_dir();
                if is_dir {
                    dirs.push(PickerEntry {
                        name,
                        path,
                        is_dir: true,
                    });
                } else if picker.show_all
                    || picker.mode == PickerMode::SaveDump
                    || is_picker_target_file(&path, picker.mode)
                {
                    files.push(PickerEntry {
                        name,
                        path,
                        is_dir: false,
                    });
                }
            }
        }
        Err(e) => picker.error = Some(tt!("无法读取目录: {}", e)),
    }

    dirs.sort_by_key(|a| a.name.to_lowercase());
    files.sort_by_key(|a| a.name.to_lowercase());

    picker.entries.extend(dirs);
    picker.entries.extend(files);
}
