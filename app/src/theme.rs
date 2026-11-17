//! Colores al estilo "Dark+" de VS Code.

use egui::{Color32, CornerRadius, FontId, Stroke, TextStyle};
use riscode_asm::highlight::Class;

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

pub const EDITOR_BG: Color32 = hex(0x1E1E1E);
pub const SIDEBAR_BG: Color32 = hex(0x252526);
pub const ACTIVITY_BG: Color32 = hex(0x333333);
pub const TITLE_BG: Color32 = hex(0x3C3C3C);
pub const TAB_INACTIVE: Color32 = hex(0x2D2D2D);
pub const BORDER: Color32 = hex(0x2B2B2B);
pub const STATUS_IDLE: Color32 = hex(0x007ACC);
pub const STATUS_RUNNING: Color32 = hex(0xCC6633);
pub const STATUS_ERROR: Color32 = hex(0xA1260D);
pub const STATUS_DONE: Color32 = hex(0x16825D);
pub const FG: Color32 = hex(0xCCCCCC);
pub const FG_DIM: Color32 = hex(0x858585);
pub const FG_BRIGHT: Color32 = hex(0xFFFFFF);
pub const ACCENT: Color32 = hex(0x0E639C);
pub const ACCENT_BRIGHT: Color32 = hex(0x3794FF);
pub const SELECTION: Color32 = hex(0x264F78);
pub const CURRENT_LINE: Color32 = hex(0x2A2D2E);
pub const EXEC_LINE: Color32 = Color32::from_rgba_premultiplied(64, 64, 0, 90);
pub const EXEC_ARROW: Color32 = hex(0xFFCC00);
pub const ERROR_LINE: Color32 = Color32::from_rgba_premultiplied(90, 20, 20, 110);
pub const BREAKPOINT: Color32 = hex(0xE51400);
pub const ERROR: Color32 = hex(0xF14C4C);
pub const WARNING: Color32 = hex(0xCCA700);
pub const CHANGED: Color32 = Color32::from_rgba_premultiplied(14, 99, 156, 120);

pub fn syntax(c: Class) -> Color32 {
    match c {
        Class::Text | Class::Punct => hex(0xD4D4D4),
        Class::Comment => hex(0x6A9955),
        Class::Instruction => hex(0x569CD6),
        Class::Pseudo => hex(0xC586C0),
        Class::Directive => hex(0xD7BA7D),
        Class::Register => hex(0x9CDCFE),
        Class::Number => hex(0xB5CEA8),
        Class::String => hex(0xCE9178),
        Class::LabelDef => hex(0xDCDCAA),
        Class::LabelRef => hex(0x4EC9B0),
        Class::Constant => hex(0x4FC1FF),
        Class::Unknown => hex(0xF44747),
    }
}

pub const MONO_SIZE: f32 = 14.0;

pub fn mono() -> FontId {
    FontId::monospace(MONO_SIZE)
}

pub fn apply(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = EDITOR_BG;
    v.window_fill = SIDEBAR_BG;
    v.extreme_bg_color = hex(0x3C3C3C);
    v.faint_bg_color = hex(0x2A2A2A);
    v.code_bg_color = EDITOR_BG;
    v.window_stroke = Stroke::new(1.0, hex(0x454545));
    v.selection.bg_fill = SELECTION;
    v.selection.stroke = Stroke::new(1.0, FG_BRIGHT);
    v.hyperlink_color = ACCENT_BRIGHT;
    v.override_text_color = Some(FG);
    v.window_corner_radius = CornerRadius::same(4);
    v.menu_corner_radius = CornerRadius::same(4);
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, FG);
    v.widgets.inactive.weak_bg_fill = hex(0x3A3D41);
    v.widgets.inactive.bg_fill = hex(0x3A3D41);
    v.widgets.hovered.weak_bg_fill = hex(0x45494E);
    v.widgets.hovered.bg_fill = hex(0x45494E);
    v.widgets.active.weak_bg_fill = ACCENT;
    v.widgets.active.bg_fill = ACCENT;
    for w in [&mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        w.corner_radius = CornerRadius::same(2);
    }
    ctx.set_visuals(v);
    ctx.global_style_mut(|s| {
        s.text_styles.insert(TextStyle::Monospace, mono());
        s.text_styles.insert(TextStyle::Body, FontId::proportional(13.5));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(13.5));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(11.0));
        s.spacing.item_spacing = egui::vec2(6.0, 4.0);
        s.spacing.button_padding = egui::vec2(8.0, 3.0);
        s.spacing.scroll.bar_width = 10.0;
        s.spacing.scroll.floating = true;
    });
}
