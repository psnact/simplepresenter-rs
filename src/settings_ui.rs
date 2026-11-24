//! Settings dialog sections, laid out like the original app's group boxes and forms.

use egui::{RichText, Ui};

use crate::settings::{
    Background, BackgroundKind, HAlign, OverlayConfig, RefPos, Rgba, TextStyle, VAlign,
};

/// A titled, bordered section like a Qt group box.
pub fn group<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::group(ui.style())
        .corner_radius(8.0)
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).strong());
            ui.add_space(4.0);
            add(ui)
        })
        .inner
}

/// A two-column form with right-aligned labels like a Qt form layout.
pub fn form(ui: &mut Ui, id: &str, add: impl FnOnce(&mut Form<'_>)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 8.0])
        .min_col_width(150.0)
        .show(ui, |ui| {
            let mut f = Form { ui };
            add(&mut f);
        });
}

pub struct Form<'a> {
    pub ui: &'a mut Ui,
}

impl Form<'_> {
    pub fn row<R>(&mut self, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
        self.ui
            .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(label)
            });
        let r = self.ui.horizontal(add).inner;
        self.ui.end_row();
        r
    }

    pub fn note(&mut self, text: &str) {
        self.ui.label("");
        self.ui.label(RichText::new(text).italics().weak());
        self.ui.end_row();
    }
}

pub fn note(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).weak()).wrap());
}

/// A launcher section: a button that opens a detail dialog, plus a description.
pub fn launcher(ui: &mut Ui, title: &str, button: &str, text: &str) -> bool {
    group(ui, title, |ui| {
        let clicked = ui.button(button).clicked();
        note(ui, text);
        clicked
    })
}

/// Edits only the RGB part; opacity has its own slider, so a fully transparent
/// colour keeps its hue instead of collapsing to black.
fn color(ui: &mut Ui, c: &mut Rgba) {
    let [r, g, b, _] = &mut c.0;
    let mut rgb = [*r, *g, *b];
    if ui.color_edit_button_srgb(&mut rgb).changed() {
        [*r, *g, *b] = rgb;
    }
}

fn percent(ui: &mut Ui, alpha: &mut u8) {
    let mut p = (*alpha as f32 / 255.0 * 100.0).round();
    if ui
        .add(egui::Slider::new(&mut p, 0.0..=100.0).suffix("%"))
        .changed()
    {
        *alpha = (p / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8;
    }
}

fn combo<T: PartialEq + Copy>(ui: &mut Ui, id: &str, value: &mut T, items: &[(T, &str)]) {
    let current = items
        .iter()
        .find(|(v, _)| v == value)
        .map(|(_, l)| *l)
        .unwrap_or("");
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .width(160.0)
        .show_ui(ui, |ui| {
            for (v, l) in items {
                ui.selectable_value(value, *v, *l);
            }
        });
}

const ALIGN: [(HAlign, &str); 3] = [
    (HAlign::Left, "Left"),
    (HAlign::Center, "Center"),
    (HAlign::Right, "Right"),
];
const VPOS: [(VAlign, &str); 3] = [
    (VAlign::Top, "Top"),
    (VAlign::Center, "Center"),
    (VAlign::Bottom, "Bottom"),
];

fn font_row(f: &mut Form<'_>, st: &mut TextStyle, families: &[String], id: &str, label: &str) {
    f.row(label, |ui| {
        egui::ComboBox::from_id_salt((id, "font"))
            .selected_text(st.family.clone())
            .width(200.0)
            .show_ui(ui, |ui| {
                for fam in families {
                    ui.selectable_value(&mut st.family, fam.clone(), fam);
                }
            });
        ui.toggle_value(&mut st.bold, RichText::new("B").strong())
            .on_hover_text("Bold");
        ui.toggle_value(&mut st.italic, RichText::new("I").italics())
            .on_hover_text("Italic");
        ui.toggle_value(&mut st.underline, RichText::new("U").underline())
            .on_hover_text("Underline");
        ui.toggle_value(&mut st.uppercase, "AA")
            .on_hover_text("All caps");
    });
}

fn size_row(f: &mut Form<'_>, label: &str, size: &mut f32) {
    f.row(label, |ui| {
        ui.add(
            egui::DragValue::new(size)
                .range(12.0..=200.0)
                .speed(0.5)
                .suffix(" pt"),
        )
    });
}

fn shadow(ui: &mut Ui, st: &mut TextStyle, id: &str) {
    group(ui, "Shadow", |ui| {
        form(ui, &format!("{id}-shadow"), |f| {
            f.row("", |ui| ui.checkbox(&mut st.shadow, "Text Shadow"));
            let on = st.shadow;
            f.row("Shadow Offset:", |ui| {
                ui.add_enabled_ui(on, |ui| {
                    ui.label("X");
                    ui.add(egui::DragValue::new(&mut st.shadow_offset[0]).range(-50.0..=50.0));
                    ui.label("Y");
                    ui.add(egui::DragValue::new(&mut st.shadow_offset[1]).range(-50.0..=50.0));
                });
            });
            f.row("Shadow Blur:", |ui| {
                ui.add_enabled(
                    on,
                    egui::DragValue::new(&mut st.shadow_blur)
                        .range(0.0..=50.0)
                        .suffix(" px"),
                )
            });
            f.row("Shadow Opacity:", |ui| {
                ui.add_enabled_ui(on, |ui| percent(ui, &mut st.shadow_color.0[3]))
            });
            f.row("Shadow Color:", |ui| {
                ui.add_enabled_ui(on, |ui| color(ui, &mut st.shadow_color))
            });
        });
    });
}

fn outline(ui: &mut Ui, st: &mut TextStyle, id: &str) {
    group(ui, "Outline", |ui| {
        form(ui, &format!("{id}-outline"), |f| {
            f.row("", |ui| ui.checkbox(&mut st.outline, "Text Outline/Stroke"));
            let on = st.outline;
            f.row("Outline Width:", |ui| {
                ui.add_enabled(
                    on,
                    egui::DragValue::new(&mut st.outline_width)
                        .range(1.0..=10.0)
                        .suffix(" px"),
                )
            });
            f.row("Outline Color:", |ui| {
                ui.add_enabled_ui(on, |ui| color(ui, &mut st.outline_color))
            });
        });
    });
}

fn highlight(ui: &mut Ui, st: &mut TextStyle, id: &str, width_controls: bool) {
    group(ui, "Background", |ui| {
        form(ui, &format!("{id}-hl"), |f| {
            f.row("Background:", |ui| {
                ui.checkbox(&mut st.highlight, "Background");
                ui.add_enabled_ui(st.highlight, |ui| color(ui, &mut st.highlight_color));
            });
            let on = st.highlight;
            f.row("Background Opacity:", |ui| {
                ui.add_enabled_ui(on, |ui| percent(ui, &mut st.highlight_color.0[3]))
            });
            f.row("Corner Radius:", |ui| {
                ui.add_enabled(
                    on,
                    egui::Slider::new(&mut st.highlight_radius, 0.0..=100.0).suffix(" px"),
                )
            });
            // The reference background always sizes itself to the text (with extra side padding).
            if width_controls {
                f.row("Width:", |ui| {
                    ui.add_enabled_ui(on, |ui| {
                        ui.checkbox(&mut st.highlight_uniform, "Same width for all backgrounds")
                    });
                });
                f.row("Fixed Width:", |ui| {
                    ui.add_enabled_ui(on && st.highlight_uniform, |ui| {
                        ui.add(egui::Slider::new(&mut st.highlight_width, 0.0..=100.0).suffix("%"));
                        if st.highlight_width <= 0.0 {
                            ui.weak("(auto)");
                        }
                    });
                });
            }
        });
    });
}

/// Line spacing as a whole percent within `min..=max`.
fn spacing_percent(ui: &mut Ui, spacing: &mut f32, min: u32, max: u32) -> bool {
    let mut pct = (*spacing * 100.0).round().clamp(min as f32, max as f32) as u32;
    let changed = ui
        .add(egui::Slider::new(&mut pct, min..=max).suffix("%"))
        .changed();
    *spacing = pct as f32 / 100.0;
    changed
}

/// "Song Lyrics Settings..." dialog body.
pub fn song_lyrics(ui: &mut Ui, cfg: &mut OverlayConfig, families: &[String]) {
    let id = "song";
    group(ui, "Typography", |ui| {
        form(ui, "song-typo", |f| {
            font_row(f, &mut cfg.text, families, id, "Font:");
            size_row(f, "Font Size:", &mut cfg.text.size);
            f.row("Text Color:", |ui| color(ui, &mut cfg.text.color));
            f.row("Line Spacing:", |ui| {
                spacing_percent(ui, &mut cfg.line_spacing, 100, 200)
            });
            f.row("Text Alignment:", |ui| {
                combo(ui, "song-align", &mut cfg.text.align, &ALIGN)
            });
            f.row("Vertical Position:", |ui| {
                combo(ui, "song-vpos", &mut cfg.vertical, &VPOS)
            });
        });
    });
    shadow(ui, &mut cfg.text, id);
    outline(ui, &mut cfg.text, id);
    highlight(ui, &mut cfg.text, id, true);
    enforce_song_border(cfg);
    let allow = border_allowed(cfg);
    group(ui, "Border", |ui| {
        form(ui, "song-border", |f| {
            f.row("Border:", |ui| {
                ui.add_enabled(
                    allow,
                    egui::Checkbox::new(&mut cfg.border, "Draw border around highlight"),
                )
            });
            let on = allow && cfg.border;
            f.row("Border Color:", |ui| {
                ui.add_enabled_ui(on, |ui| color(ui, &mut cfg.border_color))
            });
            f.row("Border Thickness:", |ui| {
                ui.add_enabled(
                    on,
                    egui::Slider::new(&mut cfg.border_thickness, 1.0..=20.0).suffix(" px"),
                )
            });
            f.row("Horizontal Padding:", |ui| {
                ui.add_enabled(
                    on,
                    egui::Slider::new(&mut cfg.border_padding[0], 0.0..=400.0).suffix(" px"),
                )
            });
            f.row("Vertical Padding:", |ui| {
                ui.add_enabled(
                    on,
                    egui::Slider::new(&mut cfg.border_padding[1], 0.0..=400.0).suffix(" px"),
                )
            });
            if !allow {
                f.note("The border is only available at 100% line spacing.");
            }
        });
    });
}

/// The song border needs 100% line spacing, as in the original.
pub fn border_allowed(cfg: &OverlayConfig) -> bool {
    (cfg.line_spacing * 100.0).round() as i32 == 100
}

pub fn enforce_song_border(cfg: &mut OverlayConfig) {
    if !border_allowed(cfg) {
        cfg.border = false;
    }
}

/// "Bible Text Settings..." dialog body.
pub fn bible_text(ui: &mut Ui, cfg: &mut OverlayConfig, families: &[String]) {
    let id = "bible";
    group(ui, "Typography", |ui| {
        form(ui, "bible-typo", |f| {
            font_row(f, &mut cfg.text, families, id, "Font:");
            size_row(f, "Font Size:", &mut cfg.text.size);
            f.row("Text Color:", |ui| color(ui, &mut cfg.text.color));
        });
    });
    shadow(ui, &mut cfg.text, id);
    outline(ui, &mut cfg.text, id);
    highlight(ui, &mut cfg.text, id, true);
    group(ui, "Layout", |ui| {
        form(ui, "bible-layout", |f| {
            f.row("Text Alignment:", |ui| {
                combo(ui, "bible-align", &mut cfg.text.align, &ALIGN)
            });
            f.row("Vertical Position:", |ui| {
                combo(ui, "bible-vpos", &mut cfg.vertical, &VPOS)
            });
        });
    });
}

/// Reference part of "Bible Reference Settings..." (translation and import are added by the caller).
pub fn bible_reference(ui: &mut Ui, cfg: &mut OverlayConfig, families: &[String]) {
    let id = "bibleref";
    group(ui, "Typography", |ui| {
        form(ui, "ref-typo", |f| {
            font_row(f, &mut cfg.reference, families, id, "Reference Font:");
            size_row(f, "Reference Size:", &mut cfg.reference.size);
            f.row("Reference Color:", |ui| color(ui, &mut cfg.reference.color));
        });
    });
    shadow(ui, &mut cfg.reference, id);
    outline(ui, &mut cfg.reference, id);
    highlight(ui, &mut cfg.reference, id, true);
    group(ui, "Layout", |ui| {
        form(ui, "ref-layout", |f| {
            f.row("Reference Alignment:", |ui| {
                combo(ui, "ref-align", &mut cfg.reference.align, &ALIGN)
            });
            f.row("Reference Position:", |ui| {
                combo(
                    ui,
                    "ref-pos",
                    &mut cfg.reference_pos,
                    &[
                        (RefPos::Above, "Above Main Text"),
                        (RefPos::Below, "Below Main Text"),
                    ],
                )
            });
            f.row("Separate Reference Area:", |ui| {
                ui.checkbox(
                    &mut cfg.separate_reference_area,
                    "Place reference in separate bottom area",
                )
            });
            f.row("Display Bible Version:", |ui| {
                ui.checkbox(
                    &mut cfg.display_version,
                    "Display Bible Version beside reference",
                )
            });
        });
    });
}

/// OBS "Text Formatting Settings..." dialog body.
pub fn obs_text(ui: &mut Ui, cfg: &mut OverlayConfig, families: &[String]) {
    let id = "obs";
    group(ui, "Typography", |ui| {
        form(ui, "obs-typo", |f| {
            font_row(f, &mut cfg.text, families, id, "Font:");
            size_row(f, "Font Size:", &mut cfg.text.size);
            f.row("Text Color:", |ui| color(ui, &mut cfg.text.color));
            f.row("Line Spacing:", |ui| {
                spacing_percent(ui, &mut cfg.line_spacing, 50, 200)
            });
        });
    });
    shadow(ui, &mut cfg.text, id);
    outline(ui, &mut cfg.text, id);
    highlight(ui, &mut cfg.text, id, false);
}

/// OBS "Bible Reference Formatting Settings..." dialog body.
pub fn obs_reference(ui: &mut Ui, cfg: &mut OverlayConfig, families: &[String]) {
    let id = "obsref";
    group(ui, "Typography", |ui| {
        form(ui, "obsref-typo", |f| {
            font_row(f, &mut cfg.reference, families, id, "Font:");
            size_row(f, "Font Size:", &mut cfg.reference.size);
            f.row("Text Color:", |ui| color(ui, &mut cfg.reference.color));
        });
    });
    shadow(ui, &mut cfg.reference, id);
    outline(ui, &mut cfg.reference, id);
    highlight(ui, &mut cfg.reference, id, false);
    group(ui, "Layout", |ui| {
        form(ui, "obsref-layout", |f| {
            f.row("Position:", |ui| {
                combo(
                    ui,
                    "obsref-pos",
                    &mut cfg.reference_pos,
                    &[(RefPos::Below, "Below Text"), (RefPos::Above, "Above Text")],
                )
            });
            f.row("Alignment:", |ui| {
                combo(ui, "obsref-align", &mut cfg.reference.align, &ALIGN)
            });
            f.row("Space from Text:", |ui| {
                ui.add(
                    egui::DragValue::new(&mut cfg.reference_spacing)
                        .range(0.0..=200.0)
                        .suffix(" px"),
                )
            });
            f.row("Display Bible Version:", |ui| {
                ui.checkbox(
                    &mut cfg.display_version,
                    "Display Bible Version beside reference",
                )
            });
        });
    });
}

/// One background section on the Backgrounds tab (Solid Color / Image / Video, as in the original).
pub fn background(ui: &mut Ui, title: &str, bg: &mut Background, id: &str, enabled: bool) {
    if bg.kind == BackgroundKind::None {
        bg.kind = BackgroundKind::Color;
    }
    ui.add_enabled_ui(enabled, |ui| {
        group(ui, title, |ui| {
            form(ui, id, |f| {
                f.row("Background Type:", |ui| {
                    combo(
                        ui,
                        &format!("{id}-kind"),
                        &mut bg.kind,
                        &[
                            (BackgroundKind::Color, "Solid Color"),
                            (BackgroundKind::Image, "Image"),
                            (BackgroundKind::Video, "Video"),
                        ],
                    )
                });
                let kind = bg.kind;
                f.row("Background Color:", |ui| {
                    ui.add_enabled_ui(kind == BackgroundKind::Color, |ui| color(ui, &mut bg.color))
                });
                for is_img in [true, false] {
                    let on = kind
                        == if is_img {
                            BackgroundKind::Image
                        } else {
                            BackgroundKind::Video
                        };
                    f.row(
                        if is_img {
                            "Background Image:"
                        } else {
                            "Background Video:"
                        },
                        |ui| {
                            ui.add_enabled_ui(on, |ui| {
                                let path = if is_img { &mut bg.image } else { &mut bg.video };
                                ui.add(egui::TextEdit::singleline(path).desired_width(220.0));
                                if ui.button("Browse...").clicked() {
                                    let exts: &[&str] = if is_img {
                                        &["png", "jpg", "jpeg", "bmp", "gif", "webp"]
                                    } else {
                                        &["mp4", "mov", "m4v", "mkv", "webm", "avi"]
                                    };
                                    if let Some(p) = rfd::FileDialog::new()
                                        .add_filter(if is_img { "Images" } else { "Videos" }, exts)
                                        .pick_file()
                                    {
                                        *path = p.to_string_lossy().into_owned();
                                    }
                                }
                                if !is_img {
                                    ui.checkbox(&mut bg.looping, "Loop video");
                                }
                            });
                        },
                    );
                }
            });
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn song_border_needs_full_line_spacing() {
        let mut cfg = OverlayConfig {
            border: true,
            line_spacing: 1.0,
            ..OverlayConfig::default()
        };
        enforce_song_border(&mut cfg);
        assert!(cfg.border);
        cfg.line_spacing = 1.2;
        enforce_song_border(&mut cfg);
        assert!(!cfg.border);
    }
}
