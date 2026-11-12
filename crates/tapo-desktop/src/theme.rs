//! App theme: a security-monitor / IR night-vision look applied via egui Visuals.
//! Charcoal teal-tinted surfaces, one cyan-teal accent for action/selection, status colors for
//! camera state, tight spacing, 4px rounding, flat with hairline borders.

use egui::{Color32, Context, Rounding, Stroke, Vec2};

const BG: Color32 = Color32::from_rgb(0x0E, 0x14, 0x16); // panels (near-black, teal-tinted)
const SURFACE: Color32 = Color32::from_rgb(0x15, 0x1D, 0x20); // cards, windows, idle widgets
const SURFACE_HI: Color32 = Color32::from_rgb(0x1E, 0x29, 0x2D); // hovered widgets
const EXTREME: Color32 = Color32::from_rgb(0x0A, 0x0F, 0x11); // text inputs
const BORDER: Color32 = Color32::from_rgb(0x27, 0x34, 0x38); // hairline borders
const TEXT: Color32 = Color32::from_rgb(0xD6, 0xE0, 0xE2); // not pure white
const MUTED: Color32 = Color32::from_rgb(0x8A, 0x99, 0x9C);
const ACCENT: Color32 = Color32::from_rgb(0x2F, 0xB7, 0xC3); // cyan-teal: selection + primary action

pub fn apply(ctx: &Context) {
    let mut v = egui::Visuals::dark();
    v.override_text_color = Some(TEXT);
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = EXTREME;
    v.faint_bg_color = SURFACE;
    v.window_stroke = Stroke::new(1.0_f32, BORDER);
    v.window_rounding = Rounding::same(8.0_f32);
    v.selection.bg_fill = ACCENT.linear_multiply(0.35_f32);
    v.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    v.hyperlink_color = ACCENT;

    let r = Rounding::same(4.0_f32);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = SURFACE;
    w.noninteractive.weak_bg_fill = SURFACE;
    w.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    w.noninteractive.fg_stroke = Stroke::new(1.0_f32, MUTED);
    w.noninteractive.rounding = r;

    w.inactive.bg_fill = SURFACE;
    w.inactive.weak_bg_fill = SURFACE;
    w.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER);
    w.inactive.fg_stroke = Stroke::new(1.0_f32, TEXT);
    w.inactive.rounding = r;

    w.hovered.bg_fill = SURFACE_HI;
    w.hovered.weak_bg_fill = SURFACE_HI;
    w.hovered.bg_stroke = Stroke::new(1.0_f32, ACCENT.linear_multiply(0.6_f32));
    w.hovered.fg_stroke = Stroke::new(1.0_f32, TEXT);
    w.hovered.rounding = r;

    w.active.bg_fill = ACCENT.linear_multiply(0.5_f32);
    w.active.weak_bg_fill = ACCENT.linear_multiply(0.5_f32);
    w.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    w.active.fg_stroke = Stroke::new(1.0_f32, TEXT);
    w.active.rounding = r;

    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0_f32, 6.0_f32);
        s.spacing.button_padding = Vec2::new(10.0_f32, 5.0_f32);
        s.spacing.menu_margin = egui::Margin::same(6.0_f32);
    });
}
