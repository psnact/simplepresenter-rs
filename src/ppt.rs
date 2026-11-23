//! PowerPoint decks: imported and rendered in-process by DeckCraft (no Office or LibreOffice).

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use egui::TextureHandle;

use crate::paths;

/// Rendered slide width in pixels.
const WIDTH: f64 = 1280.0;
const MAX_FILE: u64 = 1 << 30;

pub fn library_dir() -> PathBuf {
    paths::data_dir().join("presentations")
}

pub fn is_deck(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pptx"))
}

/// Presentations in the library folder, sorted by name.
pub fn library() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(library_dir())
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| is_deck(p))
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    v
}

/// Copy a presentation into the library and return its new path.
pub fn import(src: &Path) -> Result<PathBuf, String> {
    let dir = library_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let name = src.file_name().ok_or("no file name")?;
    let dst = dir.join(name);
    if dst != src {
        std::fs::copy(src, &dst).map_err(|e| format!("Could not import {}: {e}", src.display()))?;
    }
    Ok(dst)
}

pub fn name_of(p: &Path) -> String {
    p.file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Texture key used for slide `index` (0-based) of the deck at `path`.
pub fn key(path: &Path, index: usize) -> String {
    format!("ppt:{}#{index}", path.display())
}

enum Msg {
    Count(usize),
    Slide {
        index: usize,
        image: egui::ColorImage,
        jpeg: Vec<u8>,
    },
    Error(String),
}

pub struct Deck {
    pub path: PathBuf,
    pub name: String,
    pub count: usize,
    pub slides: Vec<Option<TextureHandle>>,
    pub jpegs: Vec<Arc<Vec<u8>>>,
    pub error: Option<String>,
    rx: Option<mpsc::Receiver<Msg>>,
}

impl Deck {
    /// Start importing and rendering in the background.
    pub fn open(path: &Path) -> Deck {
        let (tx, rx) = mpsc::channel();
        let p = path.to_path_buf();
        let spawned = std::thread::Builder::new()
            .name("pptx-render".into())
            .spawn(move || render_all(&p, &tx));
        let mut d = Deck {
            path: path.to_path_buf(),
            name: name_of(path),
            count: 0,
            slides: Vec::new(),
            jpegs: Vec::new(),
            error: None,
            rx: Some(rx),
        };
        if let Err(e) = spawned {
            d.error = Some(e.to_string());
            d.rx = None;
        }
        d
    }

    pub fn rendering(&self) -> bool {
        self.rx.is_some()
    }

    pub fn rendered(&self) -> usize {
        self.slides.iter().filter(|s| s.is_some()).count()
    }

    /// Take finished slides; returns the (key, texture) pairs that arrived.
    pub fn poll(&mut self, ctx: &egui::Context) -> Vec<(String, TextureHandle)> {
        let mut out = Vec::new();
        let Some(rx) = &self.rx else { return out };
        loop {
            match rx.try_recv() {
                Ok(Msg::Count(n)) => {
                    self.count = n;
                    self.slides = vec![None; n];
                    self.jpegs = vec![Arc::new(Vec::new()); n];
                }
                Ok(Msg::Slide { index, image, jpeg }) => {
                    let k = key(&self.path, index);
                    let tex = ctx.load_texture(&k, image, egui::TextureOptions::LINEAR);
                    if let Some(s) = self.slides.get_mut(index) {
                        *s = Some(tex.clone());
                    }
                    if let Some(j) = self.jpegs.get_mut(index) {
                        *j = Arc::new(jpeg);
                    }
                    out.push((k, tex));
                }
                Ok(Msg::Error(e)) => self.error = Some(e),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    break;
                }
            }
        }
        if self.rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
        out
    }
}

fn render_all(path: &Path, tx: &mpsc::Sender<Msg>) {
    let bytes = match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_FILE => Err("file is too large".to_string()),
        Ok(_) => std::fs::read(path).map_err(|e| e.to_string()),
        Err(e) => Err(e.to_string()),
    };
    let pres = match bytes.and_then(|b| deckcraft_pptx::import(&b).map_err(|e| e.to_string())) {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(Msg::Error(format!(
                "Could not open {}: {e}",
                path.display()
            )));
            return;
        }
    };
    let n = pres.slides.len();
    if tx.send(Msg::Count(n)).is_err() {
        return;
    }
    let w = pres.slide_size.width;
    let scale = if w.is_finite() && w > 0.0 {
        WIDTH / w
    } else {
        1.0
    };
    let opts = deckcraft_render::RenderOpts {
        scale,
        ..Default::default()
    };
    for index in 0..n {
        let img = deckcraft_render::render_slide(&pres, index, &opts);
        let rgba = img.to_straight();
        let (Ok(w), Ok(h)) = (usize::try_from(img.width), usize::try_from(img.height)) else {
            continue;
        };
        if w == 0 || h == 0 || rgba.len() != w.saturating_mul(h).saturating_mul(4) {
            continue;
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
        let jpeg = encode_jpeg(&rgba, img.width, img.height).unwrap_or_default();
        if tx.send(Msg::Slide { index, image, jpeg }).is_err() {
            return;
        }
    }
}

fn encode_jpeg(rgba: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    let img = image::RgbImage::from_raw(w, h, rgb)?;
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 85)
        .encode_image(&img)
        .ok()?;
    Some(out.into_inner())
}
