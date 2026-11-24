//! Paints the projection (background + styled text) into any rect: preview, projector, output.

use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind,
    TextureHandle, Vec2,
};

use crate::fonts::FontSet;
use crate::settings::{BackgroundKind, Fit, HAlign, OverlayConfig, RefPos, TextStyle, VAlign};

/// What is on screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Content {
    #[default]
    Empty,
    /// Scripture text with its reference line ("John 3:16 (NKJV)").
    Bible {
        text: String,
        reference: String,
    },
    /// One song section.
    Lyrics {
        text: String,
        title: String,
    },
    /// A full-frame picture (image media, rendered PowerPoint slide).
    Picture {
        key: String,
    },
    /// A video or audio file playing in `media::LIVE`.
    Video {
        path: String,
        title: String,
    },
    YouTube {
        url: String,
        title: String,
    },
}

/// The design size every style value is expressed in.
pub const DESIGN: Vec2 = Vec2::new(1920.0, 1080.0);

pub struct Frame<'a> {
    pub content: &'a Content,
    pub config: &'a OverlayConfig,
    pub background: Option<&'a TextureHandle>,
    pub picture: Option<&'a TextureHandle>,
    pub black: bool,
    pub fonts: &'a FontSet,
}

pub fn fit_rect(container: Rect, size: Vec2, fit: Fit) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return container;
    }
    let s = match fit {
        Fit::Stretch => return container,
        Fit::Cover => (container.width() / size.x).max(container.height() / size.y),
        Fit::Contain => (container.width() / size.x).min(container.height() / size.y),
    };
    Rect::from_center_size(container.center(), size * s)
}

/// Largest 16:9 rect centred in `avail`.
pub fn letterbox(avail: Rect, aspect: f32) -> Rect {
    let aspect = if aspect > 0.0 { aspect } else { 16.0 / 9.0 };
    let w = avail.width().min(avail.height() * aspect);
    Rect::from_center_size(avail.center(), Vec2::new(w, w / aspect))
}

fn draw_texture(p: &Painter, rect: Rect, tex: &TextureHandle, fit: Fit) {
    let r = fit_rect(rect, tex.size_vec2(), fit);
    let p = p.with_clip_rect(rect.intersect(p.clip_rect()));
    p.image(
        tex.id(),
        r,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn job(
    text: &str,
    st: &TextStyle,
    scale: f32,
    width: f32,
    line_spacing: f32,
    fonts: &FontSet,
    bracket_italics: bool,
) -> LayoutJob {
    let text = if st.uppercase {
        text.to_uppercase()
    } else {
        text.to_string()
    };
    let size = (st.size * scale).max(1.0);
    let format = |italics: bool| TextFormat {
        font_id: FontId::new(size, fonts.family(&st.family, st.bold)),
        color: st.color.color(),
        line_height: Some(size * 1.2 * line_spacing.clamp(0.5, 3.0)),
        italics,
        underline: if st.underline {
            Stroke::new((size * 0.06).max(1.0), st.color.color())
        } else {
            Stroke::NONE
        },
        ..Default::default()
    };
    let halign = match st.align {
        HAlign::Left => Align::LEFT,
        HAlign::Center => Align::Center,
        HAlign::Right => Align::RIGHT,
    };
    let mut job = if bracket_italics && (text.contains('[') || text.contains(']')) {
        // As in the original, bracketed words are italic and the brackets are dropped.
        let mut job = LayoutJob::default();
        let mut run = String::new();
        let mut bracket = false;
        let mut flush = |job: &mut LayoutJob, run: &mut String, bracket: bool| {
            if !run.is_empty() {
                job.append(run, 0.0, format(bracket || st.italic));
                run.clear();
            }
        };
        for ch in text.chars() {
            if ch == '[' || ch == ']' {
                flush(&mut job, &mut run, bracket);
                bracket = ch == '[';
            } else {
                run.push(ch);
            }
        }
        flush(&mut job, &mut run, bracket);
        if job.is_empty() {
            LayoutJob::single_section(String::new(), format(st.italic))
        } else {
            job
        }
    } else {
        LayoutJob::single_section(text, format(st.italic))
    };
    job.wrap.max_width = width.max(10.0);
    job.halign = halign;
    job
}

struct Block {
    galley: std::sync::Arc<egui::Galley>,
    style: TextStyle,
    /// Line spacing factor; at 100% the highlight is one box around the whole block.
    spacing: f32,
}

impl Block {
    fn height(&self) -> f32 {
        self.galley.size().y
    }
}

/// Anchor x for a galley laid out with `halign` inside [left, right].
fn anchor_x(align: HAlign, left: f32, right: f32) -> f32 {
    match align {
        HAlign::Left => left,
        HAlign::Center => (left + right) / 2.0,
        HAlign::Right => right,
    }
}

fn paint_block(
    p: &Painter,
    b: &Block,
    pos: Pos2,
    scale: f32,
    opacity: f32,
    area_w: f32,
    pad_x: f32,
    pad_y: f32,
    hug: bool,
) {
    let st = &b.style;
    if st.highlight {
        let pad = Vec2::new(pad_x, pad_y) * scale;
        let radius = CornerRadius::same((st.highlight_radius * scale).clamp(0.0, 255.0) as u8);
        let rows = &b.galley.rows;
        // Union of all rows, for a single box around the block and for uniform box widths.
        let (mut left, mut right) = (f32::INFINITY, f32::NEG_INFINITY);
        let (mut top, mut bottom) = (f32::INFINITY, f32::NEG_INFINITY);
        for row in rows {
            if row.glyphs.is_empty() {
                continue;
            }
            let r = row.rect();
            left = left.min(r.left());
            right = right.max(r.right());
            top = top.min(r.top());
            bottom = bottom.max(r.bottom());
        }
        let fixed_w = if st.highlight_uniform && st.highlight_width > 0.0 {
            Some((area_w * (st.highlight_width / 100.0).clamp(0.0, 1.0)).max(1.0))
        } else {
            None
        };
        // The reference and 100% line spacing get one box around the whole block, like the
        // original; wider spacing gets a fixed-height box per line so gaps open between them.
        if hug || b.spacing <= 1.01 {
            if right > left && bottom > top {
                let (x0, x1) = if let Some(w) = fixed_w {
                    let c = (left + right) / 2.0;
                    (c - w / 2.0, c + w / 2.0)
                } else {
                    (left, right)
                };
                let r = Rect::from_min_max(Pos2::new(x0, top), Pos2::new(x1, bottom));
                p.rect_filled(
                    r.translate(pos.to_vec2()).expand2(pad),
                    radius,
                    st.highlight_color.scaled(opacity),
                );
            }
        } else {
            // Like the original: boxes keep a fixed height, so extra line spacing opens gaps
            // between them instead of stretching them.
            let fixed_h = (st.size * scale).max(1.0) * 1.2;
            for row in rows {
                if row.glyphs.is_empty() {
                    continue;
                }
                let r0 = row.rect();
                let h = fixed_h.min(r0.height());
                let y0 = r0.top();
                let (x0, x1) = if let Some(w) = fixed_w {
                    if right > left {
                        let c = (left + right) / 2.0;
                        (c - w / 2.0, c + w / 2.0)
                    } else {
                        (r0.left(), r0.right())
                    }
                } else if st.highlight_uniform && right > left {
                    (left, right)
                } else {
                    (r0.left(), r0.right())
                };
                let r = Rect::from_min_max(Pos2::new(x0, y0), Pos2::new(x1, y0 + h));
                p.rect_filled(
                    r.translate(pos.to_vec2()).expand2(pad),
                    radius,
                    st.highlight_color.scaled(opacity),
                );
            }
        }
    }
    if st.shadow {
        let off = Vec2::new(st.shadow_offset[0], st.shadow_offset[1]) * scale;
        let blur = st.shadow_blur.clamp(0.0, 50.0) * scale;
        let steps: &[(f32, f32)] = if blur < 0.5 {
            &[(0.0, 1.0)]
        } else {
            &[(1.0, 0.25), (0.6, 0.35), (0.3, 0.5), (0.0, 0.7)]
        };
        for &(k, a) in steps {
            let ring: &[Vec2] = if k == 0.0 {
                &[Vec2::ZERO]
            } else {
                &[
                    Vec2::new(1.0, 0.0),
                    Vec2::new(-1.0, 0.0),
                    Vec2::new(0.0, 1.0),
                    Vec2::new(0.0, -1.0),
                ]
            };
            for d in ring {
                p.galley_with_override_text_color(
                    pos + off + *d * blur * k,
                    b.galley.clone(),
                    st.shadow_color.scaled(a * opacity),
                );
            }
        }
    }
    if st.outline {
        let w = st.outline_width.clamp(0.5, 10.0) * scale;
        let n = 16;
        for i in 0..n {
            let ang = i as f32 / n as f32 * std::f32::consts::TAU;
            p.galley_with_override_text_color(
                pos + Vec2::angled(ang) * w,
                b.galley.clone(),
                st.outline_color.scaled(opacity),
            );
        }
    }
    p.galley_with_override_text_color(pos, b.galley.clone(), st.color.scaled(opacity));
}

pub fn paint(p: &Painter, rect: Rect, f: &Frame, _ctx: &egui::Context) {
    p.rect_filled(rect, 0.0, Color32::BLACK);
    if f.black {
        return;
    }
    let cfg = f.config;
    let scale = rect.height() / DESIGN.y;
    if let Content::YouTube { title, .. } = f.content {
        p.rect_filled(rect, 0.0, egui::Color32::BLACK);
        label(p, rect, &format!("▶ {title}"));
        return;
    }
    if let Content::Picture { .. } | Content::Video { .. } = f.content {
        if let Some(t) = f.picture {
            draw_texture(p, rect, t, Fit::Contain);
        } else if let Content::Video { title, .. } = f.content {
            label(p, rect, &format!("♪ {title}"));
        }
        return;
    }
    match cfg.background.kind {
        BackgroundKind::Color => {
            p.rect_filled(rect, 0.0, cfg.background.color.color());
        }
        BackgroundKind::Image | BackgroundKind::Video => {
            if let Some(t) = f.background {
                draw_texture(p, rect, t, cfg.background.fit);
            }
        }
        BackgroundKind::None => {}
    }
    let (main, reference) = match f.content {
        Content::Empty
        | Content::Picture { .. }
        | Content::Video { .. }
        | Content::YouTube { .. } => return,
        Content::Bible { text, reference } => (
            text.as_str(),
            cfg.show_reference
                .then_some(reference_text(reference, cfg.display_version)),
        ),
        Content::Lyrics { text, title } => {
            (text.as_str(), cfg.show_reference.then_some(title.as_str()))
        }
    };
    let pad = cfg.padding.max(0.0) * scale;
    let inner = rect.shrink(pad);
    let width = inner.width();
    let opacity = cfg.opacity.clamp(0.0, 1.0);
    // Extra space around highlight boxes, like the original's padding/2; the reference gets
    // a full padding of side space.
    let hl_pad_y = (cfg.padding / 2.0).max(0.0);
    let ref_pad_x = cfg.padding.max(0.0);
    let gap = cfg.reference_spacing.clamp(0.0, 400.0) * scale;
    let separate = cfg.separate_reference_area && reference.is_some_and(|r| !r.trim().is_empty());
    // Like the original app, shrink the main and reference fonts together until a
    // long verse fits within 85% of the canvas height, down to minimum sizes.
    let max_h = rect.height() * 0.85;
    let mut main_style = cfg.text.clone();
    let mut ref_style = cfg.reference.clone();
    let (main_b, ref_b) = loop {
        // Line spacing never goes below the point where the fixed-height highlight
        // boxes (with their normal padding) would overlap; the padding never shrinks.
        let spacing_min = if main_style.highlight {
            (1.0 + 2.0 * hl_pad_y / (main_style.size.max(1.0) * 1.2)).min(3.0)
        } else {
            1.0
        };
        let line_spacing = if cfg.line_spacing <= 1.01 {
            cfg.line_spacing
        } else {
            cfg.line_spacing.max(spacing_min).min(3.0)
        };
        let main_b = Block {
            galley: p.layout_job(job(
                main,
                &main_style,
                scale,
                width,
                line_spacing,
                f.fonts,
                true,
            )),
            style: main_style.clone(),
            spacing: line_spacing,
        };
        let ref_b = reference.filter(|r| !r.trim().is_empty()).map(|r| Block {
            galley: p.layout_job(job(r, &ref_style, scale, width, 1.0, f.fonts, false)),
            spacing: 1.0,
            style: ref_style.clone(),
        });
        let text_h = main_b.height()
            + ref_b
                .as_ref()
                .map(|b| b.height() + if separate { 0.0 } else { gap })
                .unwrap_or(0.0);
        let occupied = cfg.padding.max(0.0) * scale * 2.0 + text_h;
        let fits = occupied <= max_h
            && (!separate
                || main_b.height()
                    + ref_b.as_ref().map(|b| b.height()).unwrap_or(0.0)
                    + cfg.padding.max(0.0) * scale * 2.0
                    <= max_h);
        let at_min = main_style.size <= 16.0 && ref_style.size <= 12.0;
        if fits || at_min {
            break (main_b, ref_b);
        }
        let ratio = (max_h / occupied.max(1.0)).clamp(0.70, 0.95);
        let new_main = (main_style.size * ratio).max(16.0);
        let new_ref = (ref_style.size * ratio).max(12.0);
        if new_main == main_style.size && new_ref == ref_style.size {
            break (main_b, ref_b);
        }
        main_style.size = new_main;
        ref_style.size = new_ref;
    };
    let mut bounds = Rect::NOTHING;
    if cfg.separate_reference_area
        && let Some(r) = ref_b
    {
        let ref_top = inner.bottom() - r.height();
        // Center the main text between the canvas top and the top of the reference's
        // background box, so the space above and below the text matches. The reference's
        // highlight is one box around its text plus the vertical padding.
        let ref_box_top = ref_top - hl_pad_y * scale;
        // Center the main text between the top of the canvas and the top of the reference's
        // background box, so the space above and below the text matches.
        let area = Rect::from_min_max(
            rect.min,
            Pos2::new(rect.right(), ref_box_top.max(rect.top())),
        );
        let top = area.center().y - main_b.height() / 2.0;
        for (b, y, pad_x, pad_y, hug) in [
            (main_b, top, 12.0, hl_pad_y, false),
            (r, ref_top, ref_pad_x, hl_pad_y, true),
        ] {
            let pos = Pos2::new(anchor_x(b.style.align, inner.left(), inner.right()), y);
            bounds = bounds.union(b.galley.rect.translate(pos.to_vec2()));
            paint_block(p, &b, pos, scale, opacity, inner.width(), pad_x, pad_y, hug);
        }
        if cfg.border && bounds.is_positive() {
            let r = bounds.expand2(Vec2::new(cfg.border_padding[0], cfg.border_padding[1]) * scale);
            p.rect_stroke(
                r,
                0.0,
                Stroke::new(
                    cfg.border_thickness * scale,
                    cfg.border_color.scaled(opacity),
                ),
                StrokeKind::Outside,
            );
        }
        return;
    }
    let total = main_b.height() + ref_b.as_ref().map(|b| b.height() + gap).unwrap_or(0.0);
    let top = match cfg.vertical {
        VAlign::Top => inner.top(),
        VAlign::Center => inner.center().y - total / 2.0,
        VAlign::Bottom => inner.bottom() - total,
    };
    let pair: (Option<(Block, bool)>, Option<(Block, bool)>) = match (cfg.reference_pos, ref_b) {
        (RefPos::Above, Some(r)) => (Some((r, true)), Some((main_b, false))),
        (_, r) => (Some((main_b, false)), r.map(|b| (b, true))),
    };
    let mut y = top;
    for (b, is_reference) in [pair.0, pair.1].into_iter().flatten() {
        let x = anchor_x(b.style.align, inner.left(), inner.right());
        let pos = Pos2::new(x, y);
        bounds = bounds.union(b.galley.rect.translate(pos.to_vec2()));
        paint_block(
            p,
            &b,
            pos,
            scale,
            opacity,
            inner.width(),
            if is_reference { ref_pad_x } else { 12.0 },
            hl_pad_y,
            is_reference,
        );
        y += b.height() + gap;
    }
    if cfg.border && bounds.is_positive() {
        let r = bounds.expand2(Vec2::new(cfg.border_padding[0], cfg.border_padding[1]) * scale);
        p.rect_stroke(
            r,
            0.0,
            Stroke::new(
                cfg.border_thickness * scale,
                cfg.border_color.scaled(opacity),
            ),
            StrokeKind::Outside,
        );
    }
}

/// The Bible reference with or without its trailing " (VERSION)".
pub fn reference_text(reference: &str, show_version: bool) -> &str {
    if show_version {
        return reference;
    }
    match reference.rfind(" (") {
        Some(i) if reference.ends_with(')') => &reference[..i],
        _ => reference,
    }
}

/// Small helper used by previews for status text.
pub fn label(p: &Painter, rect: Rect, text: &str) {
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        text,
        FontId::proportional(14.0),
        Color32::from_gray(110),
    );
}

#[cfg(test)]
mod tests {
    use super::reference_text;

    #[test]
    fn bible_version_suffix() {
        assert_eq!(reference_text("John 3:16 (KJV)", true), "John 3:16 (KJV)");
        assert_eq!(reference_text("John 3:16 (KJV)", false), "John 3:16");
        assert_eq!(reference_text("John 3:16", false), "John 3:16");
    }
}
