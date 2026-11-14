//! 统一的视觉风格：配色、字体、以及复刻参考界面的自定义控件。

use std::sync::Arc;

use egui::{
    Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect, Response, RichText,
    Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2,
};

// ---------------------------------------------------------------------------
// 配色
// ---------------------------------------------------------------------------

pub const BG: Color32 = Color32::from_rgb(0xee, 0xf3, 0xfb);
pub const TITLE_BG: Color32 = Color32::from_rgb(0xf7, 0xfb, 0xff);
pub const GROUP_FILL: Color32 = Color32::from_rgb(0xf4, 0xf8, 0xfe);
pub const BORDER: Color32 = Color32::from_rgb(0xb9, 0xc7, 0xda);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(0xd3, 0xdf, 0xee);
pub const TEXT: Color32 = Color32::from_rgb(0x1d, 0x2b, 0x3a);
pub const MUTED: Color32 = Color32::from_rgb(0x5d, 0x7c, 0x9a);
pub const ACCENT: Color32 = Color32::from_rgb(0x2f, 0x74, 0xc8);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0x4a, 0x7b, 0xb7);
pub const LABEL: Color32 = Color32::from_rgb(0x24, 0x33, 0x44);

pub const BTN_FILL: Color32 = Color32::from_rgb(0xe9, 0xf1, 0xfc);
pub const BTN_HOVER: Color32 = Color32::from_rgb(0xdb, 0xe9, 0xfa);
pub const BTN_ACTIVE: Color32 = Color32::from_rgb(0xcf, 0xe1, 0xf7);
pub const BTN_BORDER: Color32 = Color32::from_rgb(0xb6, 0xc8, 0xde);
pub const BTN_DISABLED_FILL: Color32 = Color32::from_rgb(0xf0, 0xf3, 0xf7);
pub const BTN_DISABLED_TEXT: Color32 = Color32::from_rgb(0x9a, 0xa8, 0xb7);

pub const GREEN: Color32 = Color32::from_rgb(0x2e, 0xa8, 0x5c);
pub const ORANGE: Color32 = Color32::from_rgb(0xe8, 0x9b, 0x3c);
pub const RED: Color32 = Color32::from_rgb(0xd9, 0x53, 0x4f);
pub const BLUE: Color32 = Color32::from_rgb(0x3d, 0x8b, 0xe0);

pub const DROP_BORDER: Color32 = Color32::from_rgb(0x9d, 0xc2, 0xe6);
pub const DROP_BORDER_HOT: Color32 = Color32::from_rgb(0x53, 0x9c, 0xe0);
pub const DROP_FILL: Color32 = Color32::from_rgb(0xfc, 0xfe, 0xff);
pub const DROP_FILL_HOT: Color32 = Color32::from_rgb(0xef, 0xf7, 0xff);

pub const HEX_ADDRESS: Color32 = Color32::from_rgb(0x55, 0x6a, 0x80);
pub const HEX_BYTE: Color32 = Color32::from_rgb(0x3a, 0x4a, 0x5c);
pub const HEX_ASCII: Color32 = Color32::from_rgb(0x24, 0x33, 0x44);

pub const FONT_NAME: &str = "cjk";

// ---------------------------------------------------------------------------
// 字体
// ---------------------------------------------------------------------------

const CJK_FONT_CANDIDATES: &[(&str, u32)] = &[
    // Windows
    (r"C:\Windows\Fonts\msyh.ttc", 0),
    (r"C:\Windows\Fonts\msyh.ttf", 0),
    (r"C:\Windows\Fonts\Deng.ttf", 0),
    (r"C:\Windows\Fonts\simhei.ttf", 0),
    (r"C:\Windows\Fonts\simsun.ttc", 0),
    // Linux
    ("/usr/share/fonts/truetype/wqy/wqy-microhei.ttc", 0),
    ("/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc", 0),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", 0),
    ("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc", 0),
    (
        "/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf",
        0,
    ),
    (
        "/usr/share/fonts/truetype/droid/DroidSansFallbackFull.ttf",
        0,
    ),
    // macOS
    ("/System/Library/Fonts/PingFang.ttc", 0),
    ("/System/Library/Fonts/STHeiti Light.ttc", 0),
    ("/Library/Fonts/Arial Unicode.ttf", 0),
];

/// 安装中文字体（找不到时退回 egui 内置字体，界面仍可用，只是中文会显示为方块）。
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    for (path, index) in CJK_FONT_CANDIDATES {
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        let mut data = egui::FontData::from_owned(bytes);
        data.index = *index;

        fonts.font_data.insert(FONT_NAME.to_owned(), Arc::new(data));

        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .push(FONT_NAME.to_owned());
        }
        break;
    }

    ctx.set_fonts(fonts);
}

pub fn apply_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing = vec2(6.0, 6.0);
    style.spacing.button_padding = vec2(8.0, 4.0);
    style.spacing.interact_size.y = 24.0;

    let visuals = &mut style.visuals;
    visuals.dark_mode = false;
    visuals.panel_fill = BG;
    visuals.window_fill = TITLE_BG;
    visuals.extreme_bg_color = Color32::WHITE;
    visuals.faint_bg_color = Color32::from_rgb(0xf6, 0xf9, 0xfe);
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SOFT);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(0xf7, 0xfb, 0xff);
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0xf3, 0xf8, 0xff);
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BTN_BORDER);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, LABEL);
    visuals.widgets.hovered.bg_fill = BTN_HOVER;
    visuals.widgets.hovered.weak_bg_fill = BTN_HOVER;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.widgets.active.bg_fill = BTN_ACTIVE;
    visuals.widgets.active.weak_bg_fill = BTN_ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.open.bg_fill = Color32::from_rgb(0xf7, 0xfb, 0xff);
    visuals.widgets.open.weak_bg_fill = Color32::from_rgb(0xf7, 0xfb, 0xff);
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.widgets.open.fg_stroke = Stroke::new(1.0_f32, TEXT);
    visuals.window_stroke = Stroke::new(1.0_f32, BORDER);
    visuals.selection.bg_fill = Color32::from_rgb(0xc5, 0xdd, 0xf7);
    visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    visuals.text_cursor.stroke = Stroke::new(1.5_f32, ACCENT);
    visuals.window_corner_radius = CornerRadius::same(6);

    style.text_styles = [
        (
            egui::TextStyle::Small,
            FontId::new(11.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Body,
            FontId::new(13.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Button,
            FontId::new(13.5, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Heading,
            FontId::new(16.0, FontFamily::Proportional),
        ),
        (
            egui::TextStyle::Monospace,
            FontId::new(12.0, FontFamily::Monospace),
        ),
    ]
    .into();

    ctx.set_theme(egui::ThemePreference::Light);
    ctx.set_style_of(egui::Theme::Light, style.clone());
    ctx.set_style_of(egui::Theme::Dark, style.clone());
    ctx.set_style(style);
}

// ---------------------------------------------------------------------------
// 组合框（带标题的分组边框，复刻桌面端 QGroupBox 观感）
// ---------------------------------------------------------------------------

pub fn group_box(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui)) -> Response {
    let frame = egui::Frame::new()
        .fill(GROUP_FILL)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .corner_radius(CornerRadius::same(3))
        .inner_margin(egui::Margin {
            left: 9,
            right: 9,
            top: 18,
            bottom: 8,
        });

    let inner = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        add_contents(ui);
    });

    // 在边框线上盖住一小段，写上分组标题。
    let rect = inner.response.rect;
    let text = RichText::new(title).size(12.5).color(LABEL);
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), FontId::proportional(12.5), LABEL);
    let _ = text;

    let title_pos = pos2(rect.left() + 12.0, rect.top() - galley.size().y * 0.5);
    let mask = Rect::from_min_size(
        pos2(title_pos.x - 5.0, title_pos.y),
        vec2(galley.size().x + 10.0, galley.size().y),
    );
    let painter = ui.painter();
    painter.rect_filled(mask, CornerRadius::ZERO, BG);
    painter.galley(title_pos, galley, LABEL);

    inner.response
}

// ---------------------------------------------------------------------------
// 操作按钮（图标 + 文字，整体居中）
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Download,
    Check,
    Erase,
    Read,
    Reset,
    Memory,
    Stop,
    Plus,
    Gear,
}

pub fn op_button(ui: &mut Ui, height: f32, icon: Icon, label: &str, enabled: bool) -> Response {
    let width = ui.available_width();
    op_button_sized(ui, width, height, icon, label, enabled)
}

pub fn op_button_sized(
    ui: &mut Ui,
    width: f32,
    height: f32,
    icon: Icon,
    label: &str,
    enabled: bool,
) -> Response {
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), sense);

    let hovered = enabled && response.hovered();
    let pressed = enabled && response.is_pointer_button_down_on();

    let fill = if !enabled {
        BTN_DISABLED_FILL
    } else if pressed {
        BTN_ACTIVE
    } else if hovered {
        BTN_HOVER
    } else {
        BTN_FILL
    };

    let painter = ui.painter();
    painter.rect(
        rect,
        CornerRadius::same(4),
        fill,
        Stroke::new(1.0_f32, if hovered { ACCENT } else { BTN_BORDER }),
        StrokeKind::Inside,
    );

    let icon_color = if !enabled {
        BTN_DISABLED_TEXT
    } else {
        match icon {
            Icon::Check => GREEN,
            Icon::Erase => ORANGE,
            Icon::Stop => RED,
            _ => BLUE,
        }
    };
    let text_color = if enabled { LABEL } else { BTN_DISABLED_TEXT };

    let text_width = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(14.0), text_color)
        .size()
        .x;

    let icon_center = pos2(rect.center().x - text_width * 0.5 - 14.0, rect.center().y);
    paint_icon(painter, icon_center, 15.0, icon, icon_color);
    painter.text(
        pos2(rect.center().x + 9.0, rect.center().y),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        text_color,
    );

    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    }
}

pub fn paint_icon(painter: &Painter, center: Pos2, size: f32, icon: Icon, color: Color32) {
    let s = size;
    let stroke = Stroke::new(1.8_f32, color);

    match icon {
        Icon::Download => {
            // 箭头
            painter.line_segment(
                [
                    pos2(center.x, center.y - s * 0.42),
                    pos2(center.x, center.y + s * 0.16),
                ],
                stroke,
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(center.x - s * 0.24, center.y + s * 0.06),
                    pos2(center.x + s * 0.24, center.y + s * 0.06),
                    pos2(center.x, center.y + s * 0.40),
                ],
                color,
                Stroke::new(1.0_f32, color),
            ));
            // 托盘
            painter.line_segment(
                [
                    pos2(center.x - s * 0.50, center.y + s * 0.55),
                    pos2(center.x + s * 0.50, center.y + s * 0.55),
                ],
                Stroke::new(2.0_f32, color),
            );
        }
        Icon::Check => {
            painter.circle_filled(center, s * 0.55, color);
            painter.line_segment(
                [
                    pos2(center.x - s * 0.26, center.y + s * 0.01),
                    pos2(center.x - s * 0.05, center.y + s * 0.22),
                ],
                Stroke::new(2.0_f32, Color32::WHITE),
            );
            painter.line_segment(
                [
                    pos2(center.x - s * 0.05, center.y + s * 0.22),
                    pos2(center.x + s * 0.28, center.y - s * 0.22),
                ],
                Stroke::new(2.0_f32, Color32::WHITE),
            );
        }
        Icon::Erase => {
            // 橡皮
            let body = Rect::from_center_size(
                pos2(center.x + s * 0.02, center.y + s * 0.02),
                vec2(s * 0.86, s * 0.58),
            );
            painter.rect(
                body,
                CornerRadius::same(3),
                color,
                Stroke::NONE,
                StrokeKind::Inside,
            );
            painter.line_segment(
                [
                    pos2(body.left() + s * 0.10, body.bottom() - s * 0.04),
                    pos2(body.right() - s * 0.10, body.top() + s * 0.04),
                ],
                Stroke::new(2.4_f32, Color32::from_rgb(0xff, 0xfd, 0xf5)),
            );
        }
        Icon::Read => {
            let doc = Rect::from_center_size(pos2(center.x, center.y), vec2(s * 0.72, s * 0.92));
            painter.rect(
                doc,
                CornerRadius::same(2),
                color,
                Stroke::NONE,
                StrokeKind::Inside,
            );
            for i in 0..3 {
                let y = doc.top() + s * 0.22 + i as f32 * s * 0.20;
                painter.line_segment(
                    [
                        pos2(doc.left() + s * 0.12, y),
                        pos2(doc.right() - s * 0.12, y),
                    ],
                    Stroke::new(1.6_f32, Color32::WHITE),
                );
            }
        }
        Icon::Reset => {
            let r = s * 0.45;
            let points: Vec<Pos2> = (0..=36)
                .map(|i| {
                    let t = i as f32 / 36.0;
                    let angle = (-0.45 + t * 1.72) * std::f32::consts::TAU;
                    pos2(center.x + r * angle.cos(), center.y + r * angle.sin())
                })
                .collect();
            painter.add(egui::Shape::line(points, Stroke::new(2.0_f32, color)));
            let tip = pos2(center.x + r * 0.20, center.y - r * 0.98);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    pos2(tip.x - s * 0.22, tip.y - s * 0.02),
                    pos2(tip.x + s * 0.20, tip.y - s * 0.14),
                    pos2(tip.x + s * 0.16, tip.y + s * 0.20),
                ],
                color,
                Stroke::new(1.0_f32, color),
            ));
        }
        Icon::Memory => {
            let body = Rect::from_center_size(center, vec2(s * 0.74, s * 0.74));
            painter.rect(
                body,
                CornerRadius::same(2),
                Color32::TRANSPARENT,
                Stroke::new(1.7_f32, color),
                StrokeKind::Inside,
            );
            for i in 0..3 {
                let t = 0.25 + i as f32 * 0.25;
                let x = body.left() + body.width() * t;
                let y = body.top() + body.height() * t;
                painter.line_segment(
                    [pos2(x, body.top() - s * 0.13), pos2(x, body.top())],
                    Stroke::new(1.6_f32, color),
                );
                painter.line_segment(
                    [pos2(x, body.bottom()), pos2(x, body.bottom() + s * 0.13)],
                    Stroke::new(1.6_f32, color),
                );
                painter.line_segment(
                    [pos2(body.left() - s * 0.13, y), pos2(body.left(), y)],
                    Stroke::new(1.6_f32, color),
                );
                painter.line_segment(
                    [pos2(body.right(), y), pos2(body.right() + s * 0.13, y)],
                    Stroke::new(1.6_f32, color),
                );
            }
        }
        Icon::Stop => {
            painter.rect_filled(
                Rect::from_center_size(center, vec2(s * 0.62, s * 0.62)),
                CornerRadius::same(2),
                color,
            );
        }
        Icon::Plus => {
            painter.circle_stroke(center, s * 0.46, Stroke::new(1.7_f32, color));
            let arm = s * 0.22;
            let st = Stroke::new(2.0_f32, color);
            painter.line_segment(
                [
                    pos2(center.x - arm, center.y),
                    pos2(center.x + arm, center.y),
                ],
                st,
            );
            painter.line_segment(
                [
                    pos2(center.x, center.y - arm),
                    pos2(center.x, center.y + arm),
                ],
                st,
            );
        }
        Icon::Gear => {
            painter.circle_stroke(center, s * 0.30, Stroke::new(1.6_f32, color));
            painter.circle_filled(center, s * 0.10, color);
            for i in 0..8 {
                let a = i as f32 / 8.0 * std::f32::consts::TAU;
                painter.line_segment(
                    [
                        pos2(center.x + s * 0.36 * a.cos(), center.y + s * 0.36 * a.sin()),
                        pos2(center.x + s * 0.50 * a.cos(), center.y + s * 0.50 * a.sin()),
                    ],
                    Stroke::new(1.6_f32, color),
                );
            }
        }
    }
}

/// 绘制蓝色文件夹图标（可带一个 “+” 号）。
pub fn draw_folder(painter: &Painter, center: Pos2, size: f32, plus: bool) {
    let w = size;
    let h = size * 0.78;
    let back = Color32::from_rgb(0x4a, 0x90, 0xd9);
    let front = Color32::from_rgb(0x74, 0xb4, 0xea);
    let front_light = Color32::from_rgb(0x8d, 0xc4, 0xf0);

    let body = Rect::from_center_size(pos2(center.x, center.y + h * 0.10), vec2(w, h * 0.80));
    let tab = Rect::from_min_size(
        pos2(body.left() + w * 0.06, body.top() - h * 0.24),
        vec2(w * 0.40, h * 0.30),
    );

    painter.rect_filled(tab, CornerRadius::same(3), back);
    painter.rect_filled(body, CornerRadius::same(4), front);
    painter.rect_filled(
        Rect::from_min_size(
            pos2(body.left() + w * 0.05, body.top() + h * 0.16),
            vec2(w * 0.90, h * 0.52),
        ),
        CornerRadius::same(3),
        front_light,
    );

    if plus {
        let c = pos2(body.center().x, body.center().y + h * 0.02);
        let arm = w * 0.16;
        let st = Stroke::new(3.0_f32, Color32::from_rgb(0xf4, 0xfa, 0xff));
        painter.line_segment([pos2(c.x - arm, c.y), pos2(c.x + arm, c.y)], st);
        painter.line_segment([pos2(c.x, c.y - arm), pos2(c.x, c.y + arm)], st);
    }
}

// ---------------------------------------------------------------------------
// 虚线框
// ---------------------------------------------------------------------------

pub fn dashed_rect(painter: &Painter, rect: Rect, color: Color32, dash: f32, gap: f32) {
    let stroke = Stroke::new(1.4_f32, color);
    let edges = [
        (rect.left_top(), rect.right_top()),
        (rect.right_top(), rect.right_bottom()),
        (rect.right_bottom(), rect.left_bottom()),
        (rect.left_bottom(), rect.left_top()),
    ];
    for (a, b) in edges {
        painter.add(egui::Shape::dashed_line(&[a, b], stroke, dash, gap));
    }
}

/// 细分割线
pub fn hline(ui: &mut Ui, color: Color32) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 1.0), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, color);
}

/// 紧凑小按钮（导出 / 清除 / 取消 等）
pub fn small_button(ui: &mut Ui, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).size(11.5).color(LABEL))
            .fill(BTN_FILL)
            .stroke(Stroke::new(1.0_f32, BTN_BORDER))
            .corner_radius(CornerRadius::same(3))
            .min_size(vec2(0.0, 20.0)),
    )
}
