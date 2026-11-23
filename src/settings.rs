//! Persistent settings (JSON in the data folder).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rgba(pub [u8; 4]);

impl Rgba {
    pub const WHITE: Rgba = Rgba([255, 255, 255, 255]);
    pub const BLACK: Rgba = Rgba([0, 0, 0, 255]);
    pub fn with_alpha(self, a: u8) -> Rgba {
        Rgba([self.0[0], self.0[1], self.0[2], a])
    }
    pub fn color(self) -> egui::Color32 {
        let [r, g, b, a] = self.0;
        egui::Color32::from_rgba_unmultiplied(r, g, b, a)
    }
    pub fn scaled(self, f: f32) -> egui::Color32 {
        let [r, g, b, a] = self.0;
        egui::Color32::from_rgba_unmultiplied(r, g, b, (a as f32 * f.clamp(0.0, 1.0)).round() as u8)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HAlign {
    Left,
    #[default]
    Center,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VAlign {
    Top,
    Center,
    #[default]
    Bottom,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RefPos {
    Above,
    #[default]
    Below,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub family: String,
    pub size: f32,
    pub bold: bool,
    pub color: Rgba,
    pub shadow: bool,
    pub shadow_offset: [f32; 2],
    pub shadow_blur: f32,
    pub shadow_color: Rgba,
    pub outline: bool,
    pub outline_width: f32,
    pub outline_color: Rgba,
    pub highlight: bool,
    pub highlight_color: Rgba,
    pub highlight_radius: f32,
    /// Every background box spans the widest line's width.
    pub highlight_uniform: bool,
    /// Fixed width for every background box, percent of the text area (0 = widest line).
    pub highlight_width: f32,
    pub uppercase: bool,
    pub align: HAlign,
    pub italic: bool,
    pub underline: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            family: "Liberation Sans".into(),
            size: 48.0,
            bold: false,
            color: Rgba::WHITE,
            shadow: true,
            shadow_offset: [2.0, 2.0],
            shadow_blur: 6.0,
            shadow_color: Rgba([0, 0, 0, 230]),
            outline: false,
            outline_width: 2.0,
            outline_color: Rgba::BLACK,
            highlight: true,
            highlight_color: Rgba([0, 0, 0, 180]),
            highlight_radius: 0.0,
            highlight_uniform: true,
            highlight_width: 0.0,
            uppercase: false,
            align: HAlign::Center,
            italic: false,
            underline: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlayConfig {
    pub text: TextStyle,
    pub reference: TextStyle,
    pub show_reference: bool,
    pub vertical: VAlign,
    pub reference_pos: RefPos,
    /// Padding as a fraction-free pixel value at 1920x1080.
    pub padding: f32,
    pub line_spacing: f32,
    pub opacity: f32,
    pub border: bool,
    pub border_thickness: f32,
    pub border_color: Rgba,
    pub border_padding: [f32; 2],
    pub background: Background,
    pub separate_reference_area: bool,
    pub display_version: bool,
    pub reference_spacing: f32,
}

impl Default for OverlayConfig {
    fn default() -> Self {
        let reference = TextStyle {
            size: 36.0,
            shadow_blur: 4.0,
            shadow_color: Rgba([0, 0, 0, 200]),
            highlight: false,
            ..TextStyle::default()
        };
        OverlayConfig {
            text: TextStyle::default(),
            reference,
            show_reference: true,
            vertical: VAlign::Bottom,
            reference_pos: RefPos::Below,
            padding: 40.0,
            line_spacing: 1.0,
            opacity: 1.0,
            border: false,
            border_thickness: 4.0,
            border_color: Rgba::WHITE,
            border_padding: [20.0, 20.0],
            background: Background::default(),
            separate_reference_area: false,
            display_version: true,
            reference_spacing: 16.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BackgroundKind {
    None,
    #[default]
    Color,
    Image,
    Video,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Fit {
    #[default]
    Cover,
    Contain,
    Stretch,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Background {
    pub kind: BackgroundKind,
    pub color: Rgba,
    pub image: String,
    pub video: String,
    pub looping: bool,
    pub fit: Fit,
}

impl Default for Background {
    fn default() -> Self {
        Background {
            kind: BackgroundKind::Color,
            color: Rgba([0, 0, 0, 0]),
            image: String::new(),
            video: String::new(),
            looping: true,
            fit: Fit::Cover,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub bible_overlay: OverlayConfig,
    pub song_overlay: OverlayConfig,
    pub last_bible: String,
    /// "" = Automatic (first available).
    pub default_bible: String,
    pub last_service: String,
    pub projector_monitor: usize,
    pub output_size: [u32; 2],
    pub overlay_server_enabled: bool,
    pub overlay_server_port: u16,
    pub song_lines_per_slide: usize,
    pub bible_verses_per_slide: usize,
    pub notes: String,
    pub dark_mode: bool,
    pub check_updates: bool,
    pub ui_scale: f32,
    pub separate_backgrounds: bool,
    pub obs_overlay: OverlayConfig,
    pub obs_canvas: [u32; 2],
    pub pause_video_in_preview: bool,
    pub show_background_in_preview: bool,
    pub notes_background: Background,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            bible_overlay: OverlayConfig::default(),
            song_overlay: OverlayConfig {
                show_reference: false,
                vertical: VAlign::Center,
                ..OverlayConfig::default()
            },
            last_bible: String::new(),
            default_bible: String::new(),
            last_service: String::new(),
            projector_monitor: 0,
            output_size: [1920, 1080],
            overlay_server_enabled: true,
            overlay_server_port: 8080,
            song_lines_per_slide: 0,
            bible_verses_per_slide: 1,
            notes: String::new(),
            dark_mode: true,
            check_updates: true,
            ui_scale: 1.0,
            separate_backgrounds: true,
            obs_overlay: {
                let d = OverlayConfig::default();
                OverlayConfig {
                    text: TextStyle {
                        size: 42.0,
                        bold: true,
                        highlight: false,
                        ..d.text
                    },
                    reference: TextStyle {
                        size: 28.0,
                        italic: true,
                        ..d.reference
                    },
                    ..d
                }
            },
            obs_canvas: [1920, 1080],
            pause_video_in_preview: false,
            show_background_in_preview: true,
            notes_background: Background::default(),
        }
    }
}

impl Settings {
    fn path() -> std::path::PathBuf {
        crate::paths::data_dir().join("settings.json")
    }

    pub fn load() -> Settings {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let p = Self::path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(p, text).map_err(|e| e.to_string())
    }
}
