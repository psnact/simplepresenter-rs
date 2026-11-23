//! Modern look for the main window: colours, rounding, spacing and text sizes only.

use egui::{Color32, CornerRadius, FontFamily, FontId, Margin, Shadow, Stroke, TextStyle, Visuals};

pub const ACCENT: Color32 = Color32::from_rgb(37, 99, 235);
const ACCENT_DARK: Color32 = Color32::from_rgb(96, 145, 255);

pub fn accent(dark: bool) -> Color32 {
    if dark { ACCENT_DARK } else { ACCENT }
}

pub fn apply(ctx: &egui::Context, dark: bool) {
    // Pin the theme so egui doesn't follow the OS light/dark setting at startup.
    let theme = if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    };
    ctx.set_theme(theme);
    ctx.set_visuals_of(egui::Theme::Dark, visuals(dark));
    ctx.set_visuals_of(egui::Theme::Light, visuals(dark));
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (
                TextStyle::Small,
                FontId::new(11.0, FontFamily::Proportional),
            ),
            (TextStyle::Body, FontId::new(13.5, FontFamily::Proportional)),
            (
                TextStyle::Button,
                FontId::new(13.5, FontFamily::Proportional),
            ),
            (
                TextStyle::Heading,
                FontId::new(17.0, FontFamily::Proportional),
            ),
            (
                TextStyle::Monospace,
                FontId::new(12.5, FontFamily::Monospace),
            ),
        ]
        .into();
        let s = &mut style.spacing;
        s.item_spacing = egui::vec2(8.0, 6.0);
        s.button_padding = egui::vec2(10.0, 4.0);
        s.interact_size.y = 24.0;
        s.window_margin = Margin::same(12);
        s.menu_margin = Margin::same(6);
        s.combo_height = 300.0;
    });
}

fn visuals(dark: bool) -> Visuals {
    let mut v = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };
    let accent = accent(dark);
    let (panel, window, field, faint, border, text) = if dark {
        (
            Color32::from_rgb(24, 26, 31),
            Color32::from_rgb(30, 33, 39),
            Color32::from_rgb(17, 19, 23),
            Color32::from_rgb(34, 37, 44),
            Color32::from_rgb(52, 56, 66),
            Color32::from_rgb(226, 229, 235),
        )
    } else {
        (
            Color32::from_rgb(244, 246, 249),
            Color32::WHITE,
            Color32::WHITE,
            Color32::from_rgb(236, 239, 244),
            Color32::from_rgb(218, 223, 231),
            Color32::from_rgb(30, 35, 45),
        )
    };
    v.panel_fill = panel;
    v.window_fill = window;
    v.extreme_bg_color = field;
    v.faint_bg_color = faint;
    v.code_bg_color = faint;
    v.override_text_color = Some(text);
    v.hyperlink_color = accent;
    v.selection.bg_fill = accent.gamma_multiply(if dark { 0.45 } else { 0.18 });
    v.selection.stroke = Stroke::new(1.0, if dark { Color32::WHITE } else { ACCENT });
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = CornerRadius::same(8);
    v.window_stroke = Stroke::new(1.0, border);
    v.window_shadow = Shadow {
        offset: [0, 6],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 110 } else { 40 }),
    };
    v.popup_shadow = Shadow {
        offset: [0, 4],
        blur: 14,
        spread: 0,
        color: Color32::from_black_alpha(if dark { 90 } else { 30 }),
    };
    v.striped = false;
    v.text_cursor.stroke = Stroke::new(2.0, accent);

    let r = CornerRadius::same(6);
    let w = &mut v.widgets;
    w.noninteractive.bg_stroke = Stroke::new(1.0, border);
    w.noninteractive.corner_radius = r;
    w.noninteractive.fg_stroke.color = text;
    let (idle, hover, press) = if dark {
        (
            Color32::from_rgb(42, 46, 55),
            Color32::from_rgb(52, 57, 68),
            Color32::from_rgb(62, 68, 80),
        )
    } else {
        (
            Color32::from_rgb(232, 236, 242),
            Color32::from_rgb(222, 228, 237),
            Color32::from_rgb(210, 218, 230),
        )
    };
    w.inactive.bg_fill = idle;
    w.inactive.weak_bg_fill = idle;
    w.inactive.bg_stroke = Stroke::new(1.0, border);
    w.inactive.corner_radius = r;
    w.inactive.fg_stroke.color = text;
    w.hovered.bg_fill = hover;
    w.hovered.weak_bg_fill = hover;
    w.hovered.bg_stroke = Stroke::new(1.0, accent.gamma_multiply(0.6));
    w.hovered.corner_radius = r;
    w.hovered.fg_stroke.color = text;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = press;
    w.active.weak_bg_fill = press;
    w.active.bg_stroke = Stroke::new(1.0, accent);
    w.active.corner_radius = r;
    w.active.fg_stroke.color = text;
    w.active.expansion = 0.0;
    w.open.bg_fill = hover;
    w.open.weak_bg_fill = hover;
    w.open.bg_stroke = Stroke::new(1.0, accent);
    w.open.corner_radius = r;
    v
}
