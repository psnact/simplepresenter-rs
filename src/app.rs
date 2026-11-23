//! Main window: Bible/Songs tabs, playlist, live preview, history/notes, projector window.

use crate::media;

mod remote;
mod song_edit;
use std::collections::HashMap;
use std::path::PathBuf;

use egui::{
    Color32, Key, KeyboardShortcut, Modifiers, RichText, TextureHandle, Ui, ViewportBuilder,
    ViewportCommand, ViewportId,
};

use crate::bible::{self, Bible, Verse};
use crate::canvas::{self, Content, Frame};
use crate::fonts::{self, FontSet};
use crate::paths;
use crate::playlist::{Item, ItemType, Playlist};
use crate::settings::{BackgroundKind, OverlayConfig, Settings};
use crate::songs::{self, SongLibrary};

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Source {
    #[default]
    None,
    Bible {
        book: String,
        chapter: u32,
        start: u32,
        end: u32,
    },
    Song {
        title: String,
        section: usize,
    },
    Slide {
        path: PathBuf,
        index: usize,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Bible,
    Songs,
    Media,
    PowerPoint,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SideTab {
    History,
    Notes,
    YouTube,
    Google,
}

const HISTORY_TYPES: [&str; 5] = ["Bible", "Song", "Media", "YouTube", "PowerPoint"];

#[derive(Default)]
struct BibleUi {
    reference: String,
    search: String,
    results: Vec<Verse>,
    book: String,
    chapter: u32,
    selected: Option<u32>,
    focus_reference: bool,
    scroll: bool,
}

#[derive(Default)]
struct SongUi {
    search: String,
    selected: Option<String>,
    section: Option<usize>,
    focus_search: bool,
}

struct HistoryEntry {
    time: String,
    kind: &'static str,
    title: String,
    content: String,
    source: Source,
    replay: Option<Item>,
}

pub struct App {
    settings: Settings,
    fonts: FontSet,
    bibles: Vec<PathBuf>,
    bible: Option<Bible>,
    bible_ui: BibleUi,
    songs: SongLibrary,
    song_ui: SongUi,
    playlist: Playlist,
    playlist_path: Option<PathBuf>,
    playlist_sel: Option<usize>,
    playlist_dirty: bool,
    tab: Tab,
    side_tab: SideTab,
    content: Content,
    source: Source,
    black: bool,
    history: Vec<HistoryEntry>,
    history_types: [bool; 5],
    history_dups: bool,
    playlist_filter: usize,
    media_filter: usize,
    media_sel: Option<PathBuf>,
    media_folder: usize,
    media_folder_dialog: u8,
    media_folder_name: String,
    projector_open: bool,
    projector_fullscreen: bool,
    projector_pos: Option<[f32; 2]>,
    displays: Vec<crate::displays::Display>,
    textures: HashMap<String, Option<TextureHandle>>,
    status: String,
    settings_open: bool,
    settings_tab: u8,
    settings_backup: Option<Settings>,
    style_dialog: u8,
    about_open: bool,
    media: media::Media,
    media_volume: f32,
    media_loop: bool,
    bg_failed: Option<String>,
    server: Option<crate::server::Server>,
    server_error: String,
    server_media_path: String,
    server_media_ts: i64,
    server_style: Option<OverlayConfig>,
    server_style_version: u64,
    media_fullscreen: bool,
    preview_rect: Option<egui::Rect>,
    offscreen: crate::offscreen::Offscreen,
    frame_requested: f64,
    egui_ctx: egui::Context,
    deck: Option<crate::ppt::Deck>,
    ppt_library: Vec<PathBuf>,
    ppt_sel: Option<PathBuf>,
    server_slides: (PathBuf, usize),
    yt_pane: crate::browser::WebPane,
    google_pane: crate::browser::WebPane,
    player_pane: crate::browser::WebPane,
    player_http: Option<crate::browser::PlayerHttp>,
    yt_muted: bool,
    yt_mute_poll_at: f64,
    yt_url: String,
    google_url: String,
    web_rect: Option<egui::Rect>,
    player_proc: Option<std::process::Child>,
    player_for: (bool, bool),
    song_editor: Option<song_edit::SongEditor>,
    medley: Option<song_edit::MedleyBuilder>,
    confirm_delete: Option<String>,
    update_rx: Option<std::sync::mpsc::Receiver<crate::update::Outcome>>,
    update_manual: bool,
    update_found: Option<crate::update::Release>,
    saved_settings: String,
    autosave_at: f64,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> App {
        let fonts = fonts::install(&cc.egui_ctx);
        let settings = Settings::load();
        crate::theme::apply(&cc.egui_ctx, settings.dark_mode);
        let songs_dir = paths::songs_dir();
        songs::seed_songs(&songs_dir);
        let bibles = bible::available_bibles();
        let mut app = App {
            songs: SongLibrary::load_dir(&songs_dir),
            fonts,
            bibles,
            bible: None,
            bible_ui: BibleUi::default(),
            song_ui: SongUi::default(),
            playlist: Playlist::default(),
            playlist_path: None,
            playlist_sel: None,
            playlist_dirty: false,
            tab: Tab::Bible,
            side_tab: SideTab::History,
            content: Content::Empty,
            source: Source::None,
            black: false,
            history: Vec::new(),
            history_types: [true; 5],
            history_dups: true,
            playlist_filter: 0,
            media_filter: 0,
            media_sel: None,
            media_folder: 0,
            media_folder_dialog: 0,
            media_folder_name: String::new(),
            projector_open: false,
            projector_fullscreen: false,
            projector_pos: None,
            displays: crate::displays::list(),
            textures: HashMap::new(),
            status: String::new(),
            settings_open: false,
            settings_tab: 0,
            settings_backup: None,
            style_dialog: 0,
            about_open: false,
            media: media::Media::default(),
            media_volume: 1.0,
            media_loop: false,
            bg_failed: None,
            server: None,
            server_error: String::new(),
            server_media_path: String::new(),
            server_media_ts: 0,
            server_style: None,
            server_style_version: 0,
            media_fullscreen: false,
            preview_rect: None,
            offscreen: Default::default(),
            frame_requested: 0.0,
            egui_ctx: cc.egui_ctx.clone(),
            deck: None,
            ppt_library: crate::ppt::library(),
            ppt_sel: None,
            server_slides: (PathBuf::new(), 0),
            yt_muted: true,
            yt_mute_poll_at: 0.0,
            player_http: None,
            yt_pane: Default::default(),
            google_pane: Default::default(),
            player_pane: Default::default(),
            yt_url: "https://www.youtube.com".into(),
            google_url: "https://www.google.com".into(),
            web_rect: None,
            player_proc: None,
            player_for: (false, false),
            song_editor: None,
            medley: None,
            confirm_delete: None,
            update_rx: None,
            update_manual: false,
            update_found: None,
            saved_settings: String::new(),
            autosave_at: 0.0,
            settings,
        };
        app.yt_pane.set_init_script(crate::browser::YT_MUTE_GUARD);
        let def = app.settings.default_bible.clone();
        let idx = app
            .bibles
            .iter()
            .position(|p| !def.is_empty() && p.to_string_lossy() == def)
            .unwrap_or(0);
        app.load_bible(idx);
        let svc = app.settings.last_service.clone();
        let opened = std::env::args_os().skip(1).map(PathBuf::from).find(|p| {
            p.is_file()
                && p.extension().is_some_and(|e| {
                    e.eq_ignore_ascii_case("spp") || e.eq_ignore_ascii_case("service")
                })
        });
        if let Some(p) = opened {
            app.open_service(p);
        } else if !svc.is_empty() && std::path::Path::new(&svc).exists() {
            app.open_service(PathBuf::from(svc));
        }
        if let Some(e) = app.songs.errors.first() {
            app.status = e.clone();
        }
        app.start_server();
        app.egui_ctx
            .set_zoom_factor(app.settings.ui_scale.clamp(0.75, 2.0));
        if app.settings.check_updates {
            app.update_rx = Some(crate::update::check(app.egui_ctx.clone()));
        }
        app
    }

    fn load_bible(&mut self, idx: usize) {
        let Some(path) = self.bibles.get(idx).cloned() else {
            self.status = format!(
                "No Bible files found in {}",
                bible::bible_directory().display()
            );
            return;
        };
        match Bible::load(&path) {
            Ok(b) => {
                self.bible_ui.book = b.books.first().map(|b| b.name.clone()).unwrap_or_default();
                self.bible_ui.chapter = 1;
                self.bible_ui.selected = None;
                self.status = format!("Loaded {} ({})", b.translation, b.acronym);
                self.bible = Some(b);
                self.settings.last_bible = path.to_string_lossy().into_owned();
            }
            Err(e) => self.status = e,
        }
    }

    // ----- projection -----

    fn overlay(&self) -> OverlayConfig {
        match &self.content {
            Content::Lyrics { title, .. } if title == NOTES_TITLE => {
                let s = &self.settings;
                let background = if s.separate_backgrounds {
                    s.notes_background.clone()
                } else {
                    s.bible_overlay.background.clone()
                };
                OverlayConfig {
                    background,
                    show_reference: false,
                    ..s.bible_overlay.clone()
                }
            }
            Content::Lyrics { .. } => {
                let mut cfg = self.settings.song_overlay.clone();
                // The song line spacing is re-based at the user's request: the spacing that
                // the old 170% produced is now the new 100%, leaving room to adjust upward.
                cfg.line_spacing = (cfg.line_spacing * 1.7).min(3.0);
                cfg
            }
            _ => self.settings.bible_overlay.clone(),
        }
    }

    fn set_live(&mut self, content: Content, source: Source, label: String) {
        self.black = false;
        if !matches!(content, Content::Video { .. }) {
            self.media.stop(media::LIVE);
        }
        if !matches!(content, Content::YouTube { .. }) {
            self.stop_youtube();
        }
        self.content = content;
        if !label.is_empty()
            && self
                .history
                .first()
                .map(|h| h.source != source || h.title != label)
                .unwrap_or(true)
        {
            let media_item = |kind: ItemType, key: &str, value: &str| {
                let mut data = serde_json::Map::new();
                data.insert(key.into(), serde_json::Value::String(value.to_string()));
                Some(Item {
                    kind,
                    title: label.clone(),
                    reference: value.to_string(),
                    data,
                })
            };
            let (kind, text, replay) = match &self.content {
                Content::Bible { text, .. } => ("Bible", text.clone(), None),
                Content::Lyrics { text, .. } => ("Song", text.replace('\n', " / "), None),
                Content::Picture { key } if key.starts_with("ppt:") => {
                    ("PowerPoint", label.clone(), None)
                }
                Content::Picture { key } => (
                    "Media",
                    key.clone(),
                    media_item(ItemType::Media, "path", key),
                ),
                Content::Video { path, .. } => (
                    "Media",
                    path.clone(),
                    media_item(ItemType::Media, "path", path),
                ),
                Content::YouTube { url, .. } => (
                    "YouTube",
                    url.clone(),
                    media_item(ItemType::YouTube, "url", url),
                ),
                Content::Empty => ("", String::new(), None),
            };
            let entry = HistoryEntry {
                time: chrono::Local::now().format("%H:%M:%S").to_string(),
                kind,
                title: label,
                content: text,
                source: source.clone(),
                replay,
            };
            self.history.insert(0, entry);
            self.history.truncate(500);
        }
        self.source = source;
    }

    fn project_bible(&mut self, book: &str, chapter: u32, start: u32, end: u32) -> bool {
        let Some(b) = &self.bible else { return false };
        let verses = b.verses(book, chapter, start, end);
        let (Some(first), Some(last)) = (verses.first(), verses.last()) else {
            self.status = format!("Verse not found: {book} {chapter}:{start}");
            return false;
        };
        let text = if verses.len() == 1 {
            first.text.clone()
        } else {
            verses
                .iter()
                .map(|v| format!("{} {}", superscript(v.verse), v.text))
                .collect::<Vec<_>>()
                .join(" ")
        };
        let range = if first.verse == last.verse {
            format!("{}", first.verse)
        } else {
            format!("{}-{}", first.verse, last.verse)
        };
        let reference = format!("{} {}:{} ({})", first.book, chapter, range, b.acronym);
        let src = Source::Bible {
            book: first.book.clone(),
            chapter,
            start: first.verse,
            end: last.verse,
        };
        let label = format!("{} {}:{}", first.book, chapter, range);
        self.bible_ui.book = first.book.clone();
        self.bible_ui.chapter = chapter;
        self.bible_ui.selected = Some(first.verse);
        self.set_live(Content::Bible { text, reference }, src, label);
        true
    }

    fn project_reference(&mut self, reference: &str) -> bool {
        let Some(r) = self
            .bible
            .as_ref()
            .and_then(|b| b.parse_reference(reference))
        else {
            self.status = format!("Could not understand reference \"{reference}\"");
            return false;
        };
        if r.chapter_only {
            self.bible_ui.book = r.book.clone();
            self.bible_ui.chapter = r.chapter;
            return self.project_bible(&r.book, r.chapter, 1, 1);
        }
        self.project_bible(&r.book, r.chapter, r.start, r.end)
    }

    fn project_song(&mut self, title: &str, section: usize) -> bool {
        let Some(song) = self.songs.get(title) else {
            self.status = format!("Song not found: {title}");
            return false;
        };
        let Some(s) = song.sections.get(section) else {
            return false;
        };
        let content = Content::Lyrics {
            text: s.text(),
            title: song.title.clone(),
        };
        let label = format!("{} — {}", song.title, song.display_label(section));
        let title = song.title.clone();
        self.song_ui.selected = Some(title.clone());
        self.song_ui.section = Some(section);
        self.set_live(content, Source::Song { title, section }, label);
        true
    }

    fn project_source(&mut self, src: &Source) {
        match src.clone() {
            Source::None => self.clear(),
            Source::Bible {
                book,
                chapter,
                start,
                end,
            } => {
                self.project_bible(&book, chapter, start, end);
            }
            Source::Song { title, section } => {
                self.project_song(&title, section);
            }
            Source::Slide { path, index } => self.project_slide(&path, index),
        }
    }

    fn clear(&mut self) {
        self.media.stop(media::LIVE);
        self.stop_youtube();
        self.content = Content::Empty;
        self.source = Source::None;
    }

    fn step(&mut self, forward: bool) {
        match self.source.clone() {
            Source::Bible {
                book,
                chapter,
                start,
                end,
            } => {
                let Some(b) = &self.bible else { return };
                let span = (b.verses(&book, chapter, start, end).len().max(1)) as i32;
                if forward {
                    if let Some(ns) = b.neighbour_verse(&book, chapter, end, 1) {
                        let ne = b
                            .neighbour_verse(&book, chapter, ns, span - 1)
                            .filter(|_| span > 1)
                            .unwrap_or(ns);
                        self.project_bible(&book, chapter, ns, ne);
                    } else if let Some(nc) = b.next_chapter(&book, chapter)
                        && let Some(ns) = b.first_verse(&book, nc)
                    {
                        let ne = b
                            .neighbour_verse(&book, nc, ns, span - 1)
                            .filter(|_| span > 1)
                            .unwrap_or(ns);
                        self.project_bible(&book, nc, ns, ne);
                    }
                } else if let Some(ne) = b.neighbour_verse(&book, chapter, start, -1) {
                    let ns = b
                        .neighbour_verse(&book, chapter, ne, 1 - span)
                        .filter(|_| span > 1)
                        .unwrap_or(ne);
                    self.project_bible(&book, chapter, ns, ne);
                } else if let Some(pc) = b.prev_chapter(&book, chapter)
                    && let Some(ne) = b.last_verse(&book, pc)
                {
                    let ns = b
                        .neighbour_verse(&book, pc, ne, 1 - span)
                        .filter(|_| span > 1)
                        .unwrap_or(ne);
                    self.project_bible(&book, pc, ns, ne);
                }
            }
            Source::Song { title, section } => {
                let n = self
                    .songs
                    .get(&title)
                    .map(|s| s.sections.len())
                    .unwrap_or(0);
                if forward && section + 1 < n {
                    self.project_song(&title, section + 1);
                } else if !forward && section > 0 {
                    self.project_song(&title, section - 1);
                }
            }
            Source::Slide { path, index } => {
                let n = self
                    .deck
                    .as_ref()
                    .filter(|d| d.path == path)
                    .map(|d| d.count)
                    .unwrap_or(0);
                if forward && index + 1 < n {
                    self.project_slide(&path, index + 1);
                } else if !forward && index > 0 {
                    self.project_slide(&path, index - 1);
                }
            }
            Source::None => match self.tab {
                Tab::PowerPoint => {
                    if let Some(d) = &self.deck {
                        let p = d.path.clone();
                        self.project_slide(&p, 0);
                    }
                }
                Tab::Songs => {
                    if let Some(t) = self.song_ui.selected.clone() {
                        self.project_song(&t, 0);
                    }
                }
                _ => self.project_selected_verse(),
            },
        }
    }

    fn project_media(&mut self, path: &str, label: String) {
        match media::kind_of(std::path::Path::new(path)) {
            Some(media::Kind::Image) | None => self.set_live(
                Content::Picture {
                    key: path.to_string(),
                },
                Source::None,
                label,
            ),
            Some(media::Kind::Video | media::Kind::Audio) => {
                self.set_live(
                    Content::Video {
                        path: path.to_string(),
                        title: label.clone(),
                    },
                    Source::None,
                    label,
                );
                match self
                    .media
                    .play(media::LIVE, path, self.media_loop, true, self.media_volume)
                {
                    Ok(()) => self.status = format!("Playing {path}"),
                    Err(e) => self.status = e,
                }
            }
        }
    }

    /// Open `path` (if it isn't the current deck) and project slide `index` once rendered.
    fn open_deck(&mut self, path: &std::path::Path) {
        if self.deck.as_ref().is_some_and(|d| d.path == path) {
            return;
        }
        self.textures.retain(|k, _| !k.starts_with("ppt:"));
        self.deck = Some(crate::ppt::Deck::open(path));
        self.ppt_sel = Some(path.to_path_buf());
    }

    fn project_slide(&mut self, path: &std::path::Path, index: usize) {
        self.open_deck(path);
        let Some(d) = &self.deck else { return };
        if d.count > 0 && index >= d.count {
            return;
        }
        let label = format!("{} — Slide {}", d.name, index + 1);
        let key = crate::ppt::key(path, index);
        self.set_live(
            Content::Picture { key },
            Source::Slide {
                path: path.to_path_buf(),
                index,
            },
            label,
        );
    }

    fn poll_deck(&mut self, ctx: &egui::Context) {
        let Some(d) = &mut self.deck else { return };
        for (k, tex) in d.poll(ctx) {
            self.textures.insert(k, Some(tex));
        }
        if let Some(e) = d.error.take() {
            self.status = e;
        }
    }

    fn ppt_tab(&mut self, ui: &mut Ui) {
        ui.label(
            RichText::new("PowerPoint Presentations")
                .strong()
                .size(15.0),
        );
        let deck_info = self
            .deck
            .as_ref()
            .map(|d| (d.path.clone(), d.count, d.rendering(), d.rendered()));
        let current = match (&self.source, &deck_info) {
            (Source::Slide { path: p, index }, Some((dp, ..))) if p == dp => Some(*index),
            _ => None,
        };
        let mut go = None;
        ui.horizontal_wrapped(|ui| {
            if ui.button("Import...").clicked()
                && let Some(files) = rfd::FileDialog::new()
                    .add_filter("PowerPoint", &["pptx"])
                    .pick_files()
            {
                for f in files {
                    match crate::ppt::import(&f) {
                        Ok(p) => self.ppt_sel = Some(p),
                        Err(e) => self.status = e,
                    }
                }
                self.ppt_library = crate::ppt::library();
            }
            let sel = self.ppt_sel.clone();
            if ui
                .add_enabled(sel.is_some(), egui::Button::new("Remove"))
                .clicked()
                && let Some(p) = &sel
            {
                if self.deck.as_ref().is_some_and(|d| &d.path == p) {
                    if matches!(self.source, Source::Slide { .. }) {
                        self.clear();
                    }
                    self.deck = None;
                    self.textures.retain(|k, _| !k.starts_with("ppt:"));
                }
                if let Err(e) = std::fs::remove_file(p) {
                    self.status = format!("Could not remove {}: {e}", p.display());
                }
                self.ppt_sel = None;
                self.ppt_library = crate::ppt::library();
            }
            if ui
                .add_enabled(sel.is_some(), egui::Button::new("Add to Playlist"))
                .clicked()
                && let Some(p) = &sel
            {
                let mut data = serde_json::Map::new();
                data.insert(
                    "path".into(),
                    serde_json::Value::String(p.to_string_lossy().into_owned()),
                );
                self.add_to_playlist(Item {
                    kind: ItemType::PowerPoint,
                    title: crate::ppt::name_of(p),
                    reference: p.to_string_lossy().into_owned(),
                    data,
                });
            }
            let count = deck_info.as_ref().map(|d| d.1).unwrap_or(0);
            let has = deck_info.is_some();
            if ui.add_enabled(has, egui::Button::new("Start")).clicked() {
                go = Some(0);
            }
            if ui.add_enabled(has, egui::Button::new("Prev")).clicked() {
                go = Some(current.map(|c| c.saturating_sub(1)).unwrap_or(0));
            }
            if ui.add_enabled(has, egui::Button::new("Next")).clicked() {
                go = Some(
                    current
                        .map(|c| (c + 1).min(count.saturating_sub(1)))
                        .unwrap_or(0),
                );
            }
            if ui.add_enabled(has, egui::Button::new("End")).clicked() {
                go = Some(count.saturating_sub(1));
            }
            if ui.button("Open Folder").clicked() {
                let _ = std::fs::create_dir_all(crate::ppt::library_dir());
                open_folder(&crate::ppt::library_dir());
            }
            if let Some((_, n, true, done)) = &deck_info {
                ui.spinner();
                ui.label(format!("Rendering {done} of {n}"));
            }
        });
        let lib_h = (ui.available_height() * 0.3).clamp(80.0, 220.0);
        egui::ScrollArea::vertical()
            .id_salt("ppt_lib")
            .max_height(lib_h)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if self.ppt_library.is_empty() {
                    ui.weak("No presentations yet. Use Import… to add .pptx files.");
                }
                for p in self.ppt_library.clone() {
                    let open = self.deck.as_ref().is_some_and(|d| d.path == p);
                    let r = ui.selectable_label(
                        self.ppt_sel.as_ref() == Some(&p),
                        format!(
                            "{}{}",
                            if open { "▶ " } else { "" },
                            crate::ppt::name_of(&p)
                        ),
                    );
                    if r.clicked() {
                        self.ppt_sel = Some(p.clone());
                        self.open_deck(&p);
                    }
                    if r.double_clicked() {
                        self.project_slide(&p, 0);
                    }
                }
            });
        ui.separator();
        let Some(d) = &self.deck else {
            ui.weak("No presentation loaded");
            if let Some(i) = go.zip(self.ppt_sel.clone()) {
                self.project_slide(&i.1, i.0);
            }
            return;
        };
        let path = d.path.clone();
        let status = match (current, d.count) {
            (Some(c), n) => format!("Slide {} of {n}", c + 1),
            (None, n) => format!("{n} slides"),
        };
        let thumbs: Vec<Option<TextureHandle>> = d.slides.clone();
        let grid_h = (ui.available_height() - ui.spacing().interact_size.y - 4.0).max(40.0);
        egui::ScrollArea::vertical()
            .id_salt("ppt_slides")
            .auto_shrink([false, false])
            .max_height(grid_h)
            .show(ui, |ui| {
                let w = 168.0;
                let h = w * 9.0 / 16.0;
                ui.horizontal_wrapped(|ui| {
                    for (i, t) in thumbs.iter().enumerate() {
                        let (rect, resp) =
                            ui.allocate_exact_size(egui::vec2(w, h + 18.0), egui::Sense::click());
                        let img = egui::Rect::from_min_size(rect.min, egui::vec2(w, h));
                        let p = ui.painter();
                        p.rect_filled(img, 2.0, Color32::from_gray(20));
                        if let Some(t) = t {
                            let s = t.size_vec2();
                            let k = (w / s.x).min(h / s.y);
                            let r = egui::Rect::from_center_size(img.center(), s * k);
                            p.image(
                                t.id(),
                                r,
                                egui::Rect::from_min_max(
                                    egui::pos2(0.0, 0.0),
                                    egui::pos2(1.0, 1.0),
                                ),
                                Color32::WHITE,
                            );
                        }
                        let stroke = if current == Some(i) {
                            egui::Stroke::new(3.0, Color32::from_rgb(220, 60, 60))
                        } else if resp.hovered() {
                            egui::Stroke::new(2.0, Color32::from_rgb(90, 150, 230))
                        } else {
                            egui::Stroke::new(1.0, Color32::from_gray(80))
                        };
                        p.rect_stroke(img, 2.0, stroke, egui::StrokeKind::Inside);
                        p.text(
                            egui::pos2(rect.center().x, img.max.y + 9.0),
                            egui::Align2::CENTER_CENTER,
                            format!("{}", i + 1),
                            egui::FontId::proportional(12.0),
                            ui.visuals().text_color(),
                        );
                        if resp.clicked() {
                            go = Some(i);
                        }
                    }
                });
            });
        ui.label(status);
        if let Some(i) = go {
            self.project_slide(&path, i);
        }
    }

    /// URL of the localhost player page for a video (http origin, so YouTube embeds play).
    fn player_page_url(&mut self, video_url: &str, muted: bool) -> Option<String> {
        if self.player_http.is_none() {
            match crate::browser::PlayerHttp::start() {
                Ok(s) => self.player_http = Some(s),
                Err(e) => {
                    self.status = format!("YouTube player unavailable: {e}");
                    return None;
                }
            }
        }
        self.player_http.as_ref().map(|s| s.url(video_url, muted))
    }

    fn project_youtube(&mut self, url: &str, title: String) {
        if crate::browser::youtube_id(url).is_none() {
            self.status = format!("Not a YouTube video link: {url}");
            return;
        }
        let label = if title.is_empty() {
            "YouTube video".to_string()
        } else {
            title
        };
        self.set_live(
            Content::YouTube {
                url: url.to_string(),
                title: label.clone(),
            },
            Source::None,
            label,
        );
        self.restart_player();
    }

    /// (Re)start the projector's YouTube player window (a second copy of this program).
    fn restart_player(&mut self) {
        if let Some(mut c) = self.player_proc.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.player_for = (self.projector_open, self.projector_fullscreen);
        let Content::YouTube { url, .. } = &self.content else {
            return;
        };
        let video_url = url.clone();
        if !self.projector_open || !crate::browser::SUPPORTED {
            return;
        }
        let rect = self
            .egui_ctx
            .input_for(projector_id(), |i| i.viewport().outer_rect);
        let r = rect.unwrap_or(egui::Rect::from_min_size(
            egui::pos2(100.0, 100.0),
            egui::vec2(1280.0, 720.0),
        ));
        let Ok(exe) = std::env::current_exe() else {
            return;
        };
        let Some(page_url) = self.player_page_url(&video_url, false) else {
            return;
        };
        let child = std::process::Command::new(exe)
            .arg("--youtube-player")
            .arg(page_url)
            .args(
                [r.min.x, r.min.y, r.width(), r.height()].map(|v| format!("{}", v.round() as i64)),
            )
            .arg(if self.projector_fullscreen {
                "fullscreen"
            } else {
                "window"
            })
            .spawn();
        match child {
            Ok(c) => self.player_proc = Some(c),
            Err(e) => self.status = format!("Could not start the YouTube player: {e}"),
        }
    }

    fn stop_youtube(&mut self) {
        if let Some(mut c) = self.player_proc.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.player_pane.close();
    }

    fn web_tab(&mut self, ui: &mut Ui, youtube: bool) {
        if !crate::browser::SUPPORTED {
            ui.label("The built-in browser is available on Windows and macOS.");
            return;
        }
        let pane = if youtube {
            &self.yt_pane
        } else {
            &self.google_pane
        };
        let current = pane.url();
        let mut project = None;
        let mut add = None;
        ui.horizontal(|ui| {
            if ui.button("◀").on_hover_text("Back").clicked() {
                pane.script("history.back()");
            }
            if ui.button("▶").on_hover_text("Forward").clicked() {
                pane.script("history.forward()");
            }
            if ui.button("⟳").on_hover_text("Reload").clicked() {
                pane.script("location.reload()");
            }
            if youtube {
                let label = if self.yt_muted { "Muted" } else { "Sound" };
                if ui
                    .button(label)
                    .on_hover_text("Mute or unmute the video playing in this tab")
                    .clicked()
                {
                    self.yt_muted = !self.yt_muted;
                    let js = crate::browser::yt_mute_script(self.yt_muted);
                    pane.script(&js);
                }
                let video = current
                    .clone()
                    .filter(|u| crate::browser::youtube_id(u).is_some());
                if ui
                    .add_enabled(video.is_some(), egui::Button::new("Project Video"))
                    .clicked()
                {
                    project = video.clone();
                }
                if ui
                    .add_enabled(video.is_some(), egui::Button::new("+ Playlist"))
                    .clicked()
                {
                    add = video;
                }
            }
        });
        // Keep the tab's videos muted (the default) while the Mute button is on; the in-page
        // guard mutes new videos within 50ms, this poll is the backup.
        if youtube && self.yt_muted {
            let now = ui.input(|i| i.time);
            if now - self.yt_mute_poll_at >= 0.5 {
                self.yt_mute_poll_at = now;
                let js = crate::browser::yt_mute_script(true);
                pane.script(&js);
            }
        }
        if let Some(e) = &pane.error {
            ui.colored_label(Color32::LIGHT_RED, e);
        }
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, egui::Sense::hover());
        self.web_rect = Some(rect);
        if let Some(u) = project {
            // The projector takes over playback, so pause the tab's copy.
            self.yt_pane
                .script("(()=>{var v=document.querySelector('video');if(v){v.pause();}})()");
            self.project_youtube(&u, "YouTube video".into());
        }
        if let Some(u) = add {
            let mut data = serde_json::Map::new();
            data.insert("url".into(), serde_json::Value::String(u.clone()));
            self.add_to_playlist(Item {
                kind: ItemType::YouTube,
                title: "YouTube video".into(),
                reference: u,
                data,
            });
        }
    }

    /// Place, show or hide the native web views to match this frame's layout.
    fn update_webviews(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let ppp = ctx.pixels_per_point();
        let overlay_open = self.settings_open
            || self.about_open
            || self.song_editor.is_some()
            || self.medley.is_some()
            || self.confirm_delete.is_some()
            || self.update_found.is_some()
            || ctx.any_popup_open();
        if ctx.input(|i| i.pointer.any_pressed()) {
            self.yt_pane.release_focus();
            self.google_pane.release_focus();
            self.player_pane.release_focus();
        }
        let web = self.web_rect.filter(|_| !overlay_open);
        match (self.side_tab, web) {
            (SideTab::YouTube, Some(r)) => {
                let u = self.yt_url.clone();
                self.yt_pane.show(frame, r, ppp, &u);
                self.google_pane.hide();
            }
            (SideTab::Google, Some(r)) => {
                let u = self.google_url.clone();
                self.google_pane.show(frame, r, ppp, &u);
                self.yt_pane.hide();
            }
            _ => {
                self.yt_pane.hide();
                self.google_pane.hide();
            }
        }
        match (&self.content, self.preview_rect) {
            (Content::YouTube { url, .. }, Some(r)) if !overlay_open && !self.black => {
                let video_url = url.clone();
                match self.player_page_url(&video_url, self.player_proc.is_some()) {
                    Some(u) => {
                        self.player_pane.show(frame, r, ppp, &u);
                        self.player_pane.load(&u);
                    }
                    None => self.player_pane.hide(),
                }
            }
            _ => self.player_pane.hide(),
        }
        if self
            .player_proc
            .as_mut()
            .is_some_and(|c| c.try_wait().ok().flatten().is_some())
        {
            self.player_proc = None;
        }
    }

    fn project_selected_verse(&mut self) {
        let v = self.bible_ui.selected.unwrap_or(1);
        let (b, c) = (self.bible_ui.book.clone(), self.bible_ui.chapter);
        let per = self.settings.bible_verses_per_slide.max(1) as u32;
        self.project_bible(&b, c, v, v + per - 1);
    }

    fn texture(&mut self, ctx: &egui::Context, path: &str) -> Option<TextureHandle> {
        if path.is_empty() {
            return None;
        }
        if let Some(t) = self.textures.get(path) {
            return t.clone();
        }
        if path.starts_with("ppt:") {
            return None;
        }
        let tex = image::open(path).ok().map(|img| {
            let img = img.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            ctx.load_texture(
                path,
                egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()),
                egui::TextureOptions::LINEAR,
            )
        });
        if tex.is_none() {
            self.status = format!("Could not load image {path}");
        }
        self.textures.insert(path.to_string(), tex.clone());
        tex
    }

    fn paint_live(&mut self, ui: &Ui, rect: egui::Rect) {
        if ui.ctx().viewport_id() == egui::ViewportId::ROOT {
            self.preview_rect = Some(rect);
        }
        let preview = ui.ctx().viewport_id() == egui::ViewportId::ROOT;
        self.paint_live_on(ui.ctx(), ui.painter(), rect, preview);
    }

    fn paint_live_on(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        rect: egui::Rect,
        preview: bool,
    ) {
        let mut cfg = self.overlay();
        let bg = match cfg.background.kind {
            BackgroundKind::Image => self.texture(ctx, &cfg.background.image),
            BackgroundKind::Video if !cfg.background.video.is_empty() => {
                let path = cfg.background.video.clone();
                if self.bg_failed.as_deref() != Some(path.as_str())
                    && let Err(e) = self.media.ensure(media::BACKGROUND, &path)
                {
                    self.status = e;
                    self.bg_failed = Some(path);
                }
                self.media.texture(media::BACKGROUND)
            }
            _ => {
                if self.media.path(media::BACKGROUND).is_some() {
                    self.media.stop(media::BACKGROUND);
                }
                None
            }
        };
        let pic = match &self.content {
            Content::Video { .. } => self.media.texture(media::LIVE),
            Content::Picture { key } => {
                let k = key.clone();
                self.texture(ctx, &k)
            }
            _ => None,
        };
        let mut bg = bg;
        if preview && !self.settings.show_background_in_preview {
            bg = None;
            cfg.background.kind = BackgroundKind::Color;
            cfg.background.color = crate::settings::Rgba::BLACK;
        } else if preview
            && self.settings.pause_video_in_preview
            && cfg.background.kind == BackgroundKind::Video
        {
            bg = self.media.still(media::BACKGROUND).or(bg);
        }
        let frame = Frame {
            content: &self.content,
            config: &cfg,
            background: bg.as_ref(),
            picture: pic.as_ref(),
            black: self.black,
            fonts: &self.fonts,
        };
        canvas::paint(painter, rect, &frame, ctx);
    }

    // ----- playlist -----

    fn add_to_playlist(&mut self, item: Item) {
        self.playlist.items.push(item);
        self.playlist_sel = Some(self.playlist.items.len() - 1);
        self.playlist_dirty = true;
    }

    fn activate_item(&mut self, idx: usize) {
        let Some(item) = self.playlist.items.get(idx).cloned() else {
            return;
        };
        self.playlist_sel = Some(idx);
        self.project_item(item);
    }

    fn project_item(&mut self, item: Item) {
        match item.kind {
            ItemType::Bible => {
                self.tab = Tab::Bible;
                self.bible_ui.reference = item.reference.clone();
                self.project_reference(&item.reference);
            }
            ItemType::Song => {
                self.tab = Tab::Songs;
                let title = if item.reference.is_empty() {
                    item.title.clone()
                } else {
                    item.reference.clone()
                };
                if !self.project_song(&title, 0) {
                    self.project_song(&item.title, 0);
                }
            }
            ItemType::Media => {
                let path = item
                    .data
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&item.reference)
                    .to_string();
                self.project_media(&path, item.title.clone());
            }
            ItemType::PowerPoint => {
                self.tab = Tab::PowerPoint;
                let path = PathBuf::from(
                    item.data
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&item.reference),
                );
                self.project_slide(&path, 0);
            }
            ItemType::YouTube => {
                let url = item
                    .data
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or(&item.reference)
                    .to_string();
                self.project_youtube(&url, item.title.clone());
            }
        }
    }

    fn new_service(&mut self) {
        self.playlist = Playlist::default();
        self.playlist_path = None;
        self.playlist_sel = None;
        self.playlist_dirty = false;
    }

    fn open_service(&mut self, path: PathBuf) {
        match Playlist::load(&path) {
            Ok(p) => {
                self.playlist = p;
                self.status = format!("Opened {}", path.display());
                self.settings.last_service = path.to_string_lossy().into_owned();
                self.playlist_path = Some(path);
                self.playlist_sel = None;
                self.playlist_dirty = false;
            }
            Err(e) => self.status = e,
        }
    }

    fn open_service_dialog(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("SimplePresenter Playlists", &["spp", "service", "json"])
            .set_directory(paths::services_dir())
            .pick_file()
        {
            self.open_service(p);
        }
    }

    fn save_service(&mut self, save_as: bool) {
        let path = match (&self.playlist_path, save_as) {
            (Some(p), false) => Some(p.clone()),
            _ => rfd::FileDialog::new()
                .add_filter("SimplePresenter Playlists", &["spp"])
                .set_directory(paths::services_dir())
                .set_file_name("playlist.spp")
                .save_file()
                .map(|p| {
                    if p.extension().is_none() {
                        p.with_extension("spp")
                    } else {
                        p
                    }
                }),
        };
        let Some(path) = path else { return };
        match self.playlist.save(&path) {
            Ok(()) => {
                self.status = format!("Saved {}", path.display());
                self.settings.last_service = path.to_string_lossy().into_owned();
                self.playlist_path = Some(path);
                self.playlist_dirty = false;
            }
            Err(e) => self.status = e,
        }
    }

    // ----- input -----

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = |k| KeyboardShortcut::new(Modifiers::COMMAND, k);
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::N))) {
            self.new_service();
        }
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::O))) {
            self.open_service_dialog();
        }
        if ctx.input_mut(|i| {
            i.consume_shortcut(&KeyboardShortcut::new(
                Modifiers::COMMAND | Modifiers::SHIFT,
                Key::S,
            ))
        }) {
            self.save_service(true);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::S))) {
            self.save_service(false);
        }
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::B))) {
            self.tab = Tab::Bible;
            self.bible_ui.focus_reference = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::L))) {
            self.tab = Tab::Songs;
            self.song_ui.focus_search = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(Modifiers::ALT, Key::S))) {
            self.tab = Tab::Bible;
            self.bible_ui.focus_reference = true;
        }
        if ctx.input_mut(|i| i.consume_shortcut(&cmd(Key::Comma))) {
            self.settings_open = true;
        }
        if ctx.input(|i| i.key_pressed(Key::F5)) {
            self.projector_open = !self.projector_open;
            if !self.projector_open {
                self.projector_fullscreen = false;
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F11)) {
            self.toggle_projector_fullscreen(ctx);
        }
        let typing = ctx.egui_wants_keyboard_input();
        if typing {
            return;
        }
        let (next, prev, space, esc, b) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowRight)
                    || i.key_pressed(Key::ArrowDown)
                    || i.key_pressed(Key::PageDown),
                i.key_pressed(Key::ArrowLeft)
                    || i.key_pressed(Key::ArrowUp)
                    || i.key_pressed(Key::PageUp),
                i.key_pressed(Key::Space),
                i.key_pressed(Key::Escape),
                i.key_pressed(Key::B) && i.modifiers.is_none(),
            )
        });
        if next {
            self.step(true);
        }
        if prev {
            self.step(false);
        }
        if space {
            match self.tab {
                Tab::Songs => {
                    if let Some(t) = self.song_ui.selected.clone() {
                        let s = self.song_ui.section.unwrap_or(0);
                        self.project_song(&t, s);
                    }
                }
                _ => self.project_selected_verse(),
            }
        }
        if esc {
            self.clear();
        }
        if b {
            self.black = !self.black;
        }
    }

    fn toggle_projector_fullscreen(&mut self, ctx: &egui::Context) {
        if !self.projector_open {
            self.projector_open = true;
        }
        self.projector_fullscreen = !self.projector_fullscreen;
        ctx.send_viewport_cmd_to(
            projector_id(),
            ViewportCommand::Fullscreen(self.projector_fullscreen),
        );
    }

    // ----- panels -----

    fn menu_bar(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Playlist    Ctrl+N").clicked() {
                    self.new_service();
                }
                if ui.button("Open Playlist...    Ctrl+O").clicked() {
                    self.open_service_dialog();
                }
                if ui.button("Save Playlist    Ctrl+S").clicked() {
                    self.save_service(false);
                }
                if ui.button("Save Playlist As...    Ctrl+Shift+S").clicked() {
                    self.save_service(true);
                }
                ui.separator();
                if ui.button("Open Bible Folder").clicked() {
                    open_folder(&bible::bible_directory());
                }
                if ui.button("Open Songs Folder").clicked() {
                    open_folder(&paths::songs_dir());
                }
                if ui.button("Reload Songs and Bibles").clicked() {
                    self.reload_libraries();
                }
                ui.separator();
                if ui.button("Exit").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
            });
            ui.menu_button("Tools", |ui| {
                if ui.button("Clear Overlays    Esc").clicked() {
                    self.clear();
                }
                if ui.button("Settings...    Ctrl+,").clicked() {
                    self.settings_open = true;
                }
                if ui
                    .checkbox(&mut self.projector_open, "Projection Display    F5")
                    .clicked()
                    && !self.projector_open
                {
                    self.projector_fullscreen = false;
                }
                if ui.button("Projection Fullscreen    F11").clicked() {
                    self.toggle_projector_fullscreen(ui.ctx());
                }
                ui.checkbox(&mut self.black, "Black Screen    B");
                if ui.button("Bible Search    Alt+S").clicked() {
                    self.tab = Tab::Bible;
                    self.bible_ui.focus_reference = true;
                    ui.close();
                }
                ui.separator();
                if ui
                    .checkbox(
                        &mut self.settings.overlay_server_enabled,
                        "Overlay server (OBS + phone remote)",
                    )
                    .changed()
                {
                    self.start_server();
                }
                if let Some(url) = self.server_url("overlay") {
                    ui.hyperlink_to(format!("OBS overlay: {url}"), &url);
                }
                if let Some(url) = self.server_url("remote") {
                    ui.hyperlink_to(format!("Phone remote: {url}"), &url);
                }
                if !self.server_error.is_empty() {
                    ui.colored_label(Color32::LIGHT_RED, &self.server_error);
                }
                ui.checkbox(&mut self.media_fullscreen, "Media fullscreen on overlay");
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Check for Updates…").clicked() {
                    self.update_rx = Some(crate::update::check(ui.ctx().clone()));
                    self.update_manual = true;
                    self.status = "Checking for updates…".into();
                    ui.close();
                }
                if ui.button("About SimplePresenter").clicked() {
                    self.about_open = true;
                }
            });
        });
    }

    fn reload_libraries(&mut self) {
        self.songs = SongLibrary::load_dir(&paths::songs_dir());
        self.bibles = bible::available_bibles();
        let idx = self
            .bibles
            .iter()
            .position(|p| p.to_string_lossy() == self.settings.last_bible)
            .unwrap_or(0);
        self.load_bible(idx);
    }

    fn bible_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Translation:");
            let current = self
                .bible
                .as_ref()
                .map(|b| {
                    if b.acronym.is_empty() {
                        b.translation.clone()
                    } else {
                        b.acronym.clone()
                    }
                })
                .unwrap_or_else(|| "None".into());
            let mut pick = None;
            egui::ComboBox::from_id_salt("translation")
                .selected_text(current)
                .width(110.0)
                .show_ui(ui, |ui| {
                    for (i, p) in self.bibles.iter().enumerate() {
                        let name = p
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        if ui
                            .selectable_label(p.to_string_lossy() == self.settings.last_bible, name)
                            .clicked()
                        {
                            pick = Some(i);
                        }
                    }
                });
            if let Some(i) = pick {
                self.load_bible(i);
                let src = self.source.clone();
                if matches!(src, Source::Bible { .. }) {
                    self.project_source(&src);
                }
            }
        });
        ui.label("Search by Reference:");
        let r = fill_text(
            ui,
            &mut self.bible_ui.reference,
            "e.g. John 3:16 or John 3 16",
        );
        if self.bible_ui.focus_reference {
            r.request_focus();
            self.bible_ui.focus_reference = false;
        }
        if r.changed() {
            self.preview_reference();
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
            let reference = self.bible_ui.reference.clone();
            self.project_reference(&reference);
        }
        ui.label("Search in Text:");
        rtl_row(ui, |ui| {
            let search = ui.button("Search").clicked();
            let r = fill_text(
                ui,
                &mut self.bible_ui.search,
                "Search for words in Bible text...",
            );
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if search || enter {
                self.bible_ui.results = self
                    .bible
                    .as_ref()
                    .map(|b| b.search(&self.bible_ui.search, 200))
                    .unwrap_or_default();
                self.status = format!("{} results", self.bible_ui.results.len());
            }
        });
        let searching = !self.bible_ui.results.is_empty();
        let rows: Vec<Verse> = if searching {
            self.bible_ui.results.clone()
        } else {
            self.bible
                .as_ref()
                .map(|b| b.chapter(&self.bible_ui.book, self.bible_ui.chapter))
                .unwrap_or_default()
        };
        let list_h = ui.available_height() - button_row_height(ui);
        let live_color = if ui.visuals().dark_mode {
            Color32::from_rgb(255, 200, 80)
        } else {
            Color32::from_rgb(190, 90, 0)
        };
        let scroll = std::mem::take(&mut self.bible_ui.scroll);
        let mut go = None;
        let mut add = None;
        list_box(ui, "bible_results", list_h, |ui| {
            if self.bible.is_none() {
                ui.weak("No Bible loaded. Put Bible XML files in the Bible folder (File ▸ Open Bible Folder).");
            }
            for v in &rows {
                let live = matches!(&self.source, Source::Bible { book, chapter, start, end } if *book == v.book && *chapter == v.chapter && (*start..=*end).contains(&v.verse));
                let sel = !searching && self.bible_ui.selected == Some(v.verse);
                ui.horizontal(|ui| {
                    if ui
                        .small_button("+")
                        .on_hover_text("Add to playlist")
                        .clicked()
                    {
                        add = Some(v.clone());
                    }
                    let text = if searching {
                        format!("{} — {}", v.reference(), v.text)
                    } else {
                        format!("{}  {}", v.verse, v.text)
                    };
                    let text = if live {
                        RichText::new(text).color(live_color)
                    } else {
                        RichText::new(text)
                    };
                    let r = list_row(ui, sel, text);
                    if sel && scroll {
                        r.scroll_to_me(Some(egui::Align::Center));
                    }
                    if r.clicked() {
                        go = Some(v.clone());
                    }
                });
            }
        });
        if let Some(v) = go {
            self.bible_ui.reference = format!("{} {}:{}", v.book, v.chapter, v.verse);
            if searching {
                self.project_bible(&v.book, v.chapter, v.verse, v.verse);
            } else {
                self.bible_ui.selected = Some(v.verse);
                self.project_selected_verse();
            }
        }
        if let Some(v) = add {
            let reference = format!("{} {}:{}", v.book, v.chapter, v.verse);
            self.add_to_playlist(Item {
                kind: ItemType::Bible,
                title: reference.clone(),
                reference,
                data: Default::default(),
            });
        }
        match button_row(ui, &[("Previous", true), ("Project", true), ("Next", true)]) {
            Some(0) => self.step(false),
            Some(1) => {
                let reference = self.bible_ui.reference.trim().to_string();
                if reference.is_empty() || !self.project_reference(&reference) {
                    self.project_selected_verse();
                }
            }
            Some(2) => self.step(true),
            _ => {}
        }
    }

    /// Show the chapter of the reference being typed, with its verse selected.
    fn preview_reference(&mut self) {
        let Some(r) = self
            .bible
            .as_ref()
            .and_then(|b| b.parse_reference(&self.bible_ui.reference))
        else {
            return;
        };
        self.bible_ui.results.clear();
        self.bible_ui.book = r.book;
        self.bible_ui.chapter = r.chapter;
        self.bible_ui.selected = if r.chapter_only { None } else { Some(r.start) };
        self.bible_ui.scroll = true;
    }

    fn songs_tab(&mut self, ui: &mut Ui) {
        let sel = self
            .song_ui
            .selected
            .as_ref()
            .and_then(|t| self.songs.get(t))
            .cloned();
        let (mut reload, mut new_song, mut edit, mut delete, mut medley) =
            (false, false, false, false, false);
        rtl_row(ui, |ui| {
            medley = ui.button("New Medley").clicked();
            delete = ui
                .add_enabled(sel.is_some(), egui::Button::new("Delete Song"))
                .clicked();
            edit = ui
                .add_enabled(sel.is_some(), egui::Button::new("Edit Song"))
                .clicked();
            new_song = ui.button("Add Song").clicked();
            reload = ui.button("Refresh").clicked();
            let r = fill_text(ui, &mut self.song_ui.search, "Search songs...");
            if self.song_ui.focus_search {
                r.request_focus();
                self.song_ui.focus_search = false;
            }
        });
        if reload {
            self.songs = SongLibrary::load_dir(&paths::songs_dir());
            self.status = format!("{} songs", self.songs.songs.len());
        }
        if new_song {
            self.song_editor = Some(song_edit::SongEditor::new_song());
        }
        if medley {
            self.medley = Some(Default::default());
        }
        if edit && let Some(song) = &sel {
            if song.medley {
                self.medley = Some(song_edit::MedleyBuilder::edit(song));
            } else {
                self.song_editor = Some(song_edit::SongEditor::edit(song));
            }
        }
        if delete {
            self.confirm_delete = sel.as_ref().map(|s| s.title.clone());
        }
        if let Some(t) = self.confirm_delete.clone() {
            let mut keep = true;
            egui::Window::new("Delete Song")
                .collapsible(false)
                .resizable(false)
                .show(ui.ctx(), |ui| {
                    ui.label(format!("Delete \"{t}\" from the song library?"));
                    ui.horizontal(|ui| {
                        if ui.button("Delete").clicked() {
                            self.delete_song(&t);
                            keep = false;
                        }
                        if ui.button("Cancel").clicked() {
                            keep = false;
                        }
                    });
                });
            if !keep {
                self.confirm_delete = None;
            }
        }
        let titles = self.songs.search(&self.song_ui.search);
        let body_h = (ui.available_height() - button_row_height(ui)).max(60.0);
        let w = ui.available_width();
        let gap = ui.spacing().item_spacing.x;
        let lw = ((w - gap) * 0.4).max(120.0);
        let rw = (w - gap - lw).max(100.0);
        let mut pick = None;
        let mut add = None;
        let mut go = None;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(lw, body_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label("Songs:");
                    let h = ui.available_height();
                    list_box(ui, "songlist", h, |ui| {
                        for t in &titles {
                            let b = egui::Button::selectable(
                                self.song_ui.selected.as_deref() == Some(t),
                                t.as_str(),
                            )
                            .truncate()
                            .min_size(egui::vec2(ui.available_width(), 0.0));
                            let r = ui.add(b);
                            if r.clicked() {
                                pick = Some(t.clone());
                            }
                            if r.double_clicked() {
                                add = Some(t.clone());
                            }
                            r.context_menu(|ui| {
                                if ui.button("Add to playlist").clicked() {
                                    add = Some(t.clone());
                                    ui.close();
                                }
                            });
                        }
                    });
                },
            );
            ui.allocate_ui_with_layout(
                egui::vec2(rw, body_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.label("Sections:");
                    let h = ui.available_height();
                    list_box(ui, "sections", h, |ui| {
                        let Some(song) = self
                            .song_ui
                            .selected
                            .as_ref()
                            .and_then(|t| self.songs.get(t))
                        else {
                            ui.weak(format!(
                                "{} songs. Select a song to see its sections.",
                                self.songs.songs.len()
                            ));
                            return;
                        };
                        // Like the original: with labelled sections, one bold header row
                        // sits above each run of same-labelled sections, members below it.
                        let setup = song.has_labeled_sections();
                        let mut prev_label = String::new();
                        for (i, s) in song.sections.iter().enumerate() {
                            let label = song.display_label(i);
                            let kind = if s.label.is_empty() {
                                s.kind.clone()
                            } else {
                                songs::section_type_for_label(&label).replace(' ', "-")
                            };
                            let [r, g, b] = songs::section_color(&kind);
                            let col = Color32::from_rgb(r, g, b);
                            let live = self.source
                                == Source::Song {
                                    title: song.title.clone(),
                                    section: i,
                                };
                            if setup {
                                if label != prev_label {
                                    ui.add_space(3.0);
                                    ui.label(RichText::new(&label).strong().color(col));
                                    ui.add_space(3.0);
                                }
                                prev_label = label;
                                let resp = egui::Frame::group(ui.style())
                                    .fill(col)
                                    .stroke(if live {
                                        egui::Stroke::new(2.5, Color32::from_rgb(229, 57, 53))
                                    } else {
                                        egui::Stroke::NONE
                                    })
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.label(RichText::new(s.text()).color(Color32::WHITE));
                                    })
                                    .response
                                    .interact(egui::Sense::click());
                                if resp.clicked() {
                                    go = Some(i);
                                }
                                ui.add_space(2.0);
                            } else {
                                let resp = ui.add(
                                    egui::Button::selectable(live, s.text())
                                        .truncate()
                                        .min_size(egui::vec2(ui.available_width(), 0.0)),
                                );
                                if resp.clicked() {
                                    go = Some(i);
                                }
                            }
                        }
                    });
                },
            );
        });
        let has_song = self.song_ui.selected.is_some();
        let mut prev = false;
        let mut project = false;
        let mut next = false;
        ui.horizontal(|ui| {
            prev = ui
                .add_enabled(has_song, egui::Button::new("Previous"))
                .clicked();
            project = ui
                .add_enabled(has_song, egui::Button::new("Project Section"))
                .clicked();
            next = ui
                .add_enabled(has_song, egui::Button::new("Next"))
                .clicked();
            if ui
                .add_enabled(has_song, egui::Button::new("Add to Playlist"))
                .clicked()
            {
                add = self.song_ui.selected.clone();
            }
        });
        if let Some(t) = pick {
            self.song_ui.selected = Some(t);
            self.song_ui.section = None;
        }
        if let Some(i) = go
            && let Some(t) = self.song_ui.selected.clone()
        {
            self.song_ui.section = Some(i);
            self.project_song(&t, i);
        }
        if project && let Some(t) = self.song_ui.selected.clone() {
            let s = self.song_ui.section.unwrap_or(0);
            self.project_song(&t, s);
        }
        if prev || next {
            let on_song = matches!(&self.source, Source::Song { title, .. } if Some(title) == self.song_ui.selected.as_ref());
            if on_song {
                self.step(next);
            } else if let Some(t) = self.song_ui.selected.clone() {
                self.project_song(&t, 0);
            }
        }
        if let Some(t) = add {
            self.add_to_playlist(Item {
                kind: ItemType::Song,
                title: t.clone(),
                reference: t,
                data: Default::default(),
            });
        }
    }

    fn media_tab(&mut self, ui: &mut Ui) {
        const FILTERS: [&str; 4] = ["All", "Videos", "Photos", "Audio"];
        ui.label(RichText::new("Media Library").strong().size(15.0));

        let folders = Self::media_folders();
        ui.horizontal(|ui| {
            ui.label("Folder:");
            let cur = if self.media_folder == 0 {
                "All Media".to_string()
            } else {
                folders
                    .get(self.media_folder - 1)
                    .cloned()
                    .unwrap_or_default()
            };
            egui::ComboBox::from_id_salt("media_folder")
                .selected_text(&cur)
                .width(170.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.media_folder, 0, "All Media");
                    for (i, f) in folders.iter().enumerate() {
                        ui.selectable_value(&mut self.media_folder, i + 1, f);
                    }
                });
            if ui.button("New Folder").clicked() {
                self.media_folder_dialog = 1;
                self.media_folder_name.clear();
            }
            ui.add_enabled_ui(self.media_folder > 0, |ui| {
                if ui.button("Rename").clicked() {
                    self.media_folder_dialog = 2;
                    self.media_folder_name = cur.clone();
                }
                if ui.button("Delete Folder").clicked() {
                    self.media_folder_dialog = 3;
                }
            });
        });
        ui.horizontal(|ui| {
            ui.label("Filter:");
            egui::ComboBox::from_id_salt("media_filter")
                .selected_text(FILTERS[self.media_filter.min(3)])
                .width(100.0)
                .show_ui(ui, |ui| {
                    for (i, f) in FILTERS.iter().enumerate() {
                        ui.selectable_value(&mut self.media_filter, i, *f);
                    }
                });
        });

        let filter = self.media_filter;
        let folder_dir = self.media_folder_dir(&folders);
        let mut files: Vec<PathBuf> = if self.media_folder == 0 {
            let mut all: Vec<PathBuf> = std::fs::read_dir(paths::media_dir())
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            for f in &folders {
                all.extend(
                    std::fs::read_dir(paths::media_dir().join(f))
                        .into_iter()
                        .flatten()
                        .flatten()
                        .map(|e| e.path()),
                );
            }
            all
        } else {
            std::fs::read_dir(&folder_dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect()
        };
        files.retain(|p| match (media::kind_of(p), filter) {
            (None, _) => false,
            (Some(_), 0) => true,
            (Some(k), 1) => k == media::Kind::Video,
            (Some(k), 2) => k == media::Kind::Image,
            (Some(k), _) => k == media::Kind::Audio,
        });
        files.sort();

        let buttons_h = button_row_height(ui);
        let h = (ui.available_height() - buttons_h - 8.0).max(60.0);
        let list_w = (ui.available_width() * 0.4 - 6.0).max(160.0);
        let mut go = None;
        let mut add = None;
        let mut move_to = None;
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(list_w, h),
                egui::Layout::top_down(egui::Align::LEFT),
                |ui| {
                list_box(ui, "media_list", h + 8.0, |ui| {
                    if files.is_empty() {
                        ui.weak("No media yet. Use Add Media to copy pictures, videos or audio into the library.");
                    }
                    for f in &files {
                        let name = media_display_name(f);
                        let b = egui::Button::selectable(
                            self.media_sel.as_ref() == Some(f),
                            name.as_str(),
                        )
                        .truncate()
                        .min_size(egui::vec2(ui.available_width(), 0.0));
                        let r = ui.add(b);
                        if r.clicked() {
                            self.media_sel = Some(f.clone());
                        }
                        if r.double_clicked() {
                            go = Some(f.clone());
                        }
                        r.context_menu(|ui| {
                            if ui.button("Add to playlist").clicked() {
                                add = Some(f.clone());
                                ui.close();
                            }
                            if ui.button("Project").clicked() {
                                go = Some(f.clone());
                                ui.close();
                            }
                            if folders.len() > 1 {
                                ui.menu_button("Move to folder", |ui| {
                                    for (i, name) in folders.iter().enumerate() {
                                        if i + 1 == self.media_folder {
                                            continue;
                                        }
                                        if ui.button(name).clicked() {
                                            move_to = Some((f.clone(), i + 1));
                                            ui.close();
                                        }
                                    }
                                });
                            }
                        });
                    }
                });
            });
            ui.vertical(|ui| {
                self.media_preview(ui, h);
            });
        });

        // Keep the preview player on the selected video/audio file.
        let sel = self.media_sel.clone().filter(|p| p.is_file());
        let wants = sel.clone().filter(|p| {
            matches!(
                media::kind_of(p),
                Some(media::Kind::Video | media::Kind::Audio)
            )
        });
        match wants {
            Some(p) if self.media.path(media::PREVIEW) != p.to_str() => {
                match self
                    .media
                    .play(media::PREVIEW, &p.to_string_lossy(), false, false, 0.0)
                {
                    Ok(()) => self.media.toggle_pause(media::PREVIEW),
                    Err(e) => eprintln!("preview play {}: {e}", p.display()),
                }
            }
            None => self.media.stop(media::PREVIEW),
            _ => {}
        }

        match button_row(
            ui,
            &[
                ("Add Media", true),
                ("Delete Media", sel.is_some()),
                ("Add to Playlist", sel.is_some()),
            ],
        ) {
            Some(0) => {
                let all: Vec<&str> = media::IMAGE_EXTS
                    .iter()
                    .chain(media::VIDEO_EXTS)
                    .chain(media::AUDIO_EXTS)
                    .copied()
                    .collect();
                if let Some(picked) = rfd::FileDialog::new()
                    .add_filter("Media", &all)
                    .pick_files()
                {
                    let dir = self.media_folder_dir(&folders);
                    let _ = std::fs::create_dir_all(&dir);
                    for f in picked {
                        if f.parent() == Some(dir.as_path()) {
                            self.media_sel = Some(f);
                            continue;
                        }
                        let Some(dest) = unique_path(&dir, &f) else {
                            continue;
                        };
                        if let Err(e) = std::fs::copy(&f, &dest) {
                            self.status = format!("Could not add {}: {e}", f.display());
                            continue;
                        }
                        self.media_sel = Some(dest);
                    }
                }
            }
            Some(1) => {
                if let Some(p) = sel.clone() {
                    if p.starts_with(paths::media_dir()) && std::fs::remove_file(&p).is_ok() {
                        self.media_sel = None;
                        self.media.stop(media::PREVIEW);
                        self.status = format!("Deleted {}", p.display());
                    }
                }
            }
            Some(2) => add = sel.clone(),
            _ => {}
        }
        if let Some(f) = go {
            let label = media_display_name(&f);
            self.project_media(&f.to_string_lossy(), label);
        }
        if let Some((src, fi)) = move_to {
            if let (Some(dir), Some(name)) = (
                folders.get(fi - 1).map(|n| paths::media_dir().join(n)),
                src.file_name(),
            ) {
                let dest = dir.join(name);
                if std::fs::rename(&src, &dest).is_ok() {
                    self.media_sel = Some(dest);
                    self.media.stop(media::PREVIEW);
                }
            }
        }
        if let Some(f) = add {
            let name = media_display_name(&f);
            let mut data = serde_json::Map::new();
            data.insert(
                "path".into(),
                serde_json::Value::String(f.to_string_lossy().into_owned()),
            );
            self.add_to_playlist(Item {
                kind: ItemType::Media,
                title: name,
                reference: f.to_string_lossy().into_owned(),
                data,
            });
        }
    }

    fn media_folders() -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(paths::media_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                if p.is_dir() {
                    p.file_name().map(|n| n.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .collect();
        v.sort();
        v
    }

    fn media_folder_dir(&self, folders: &[String]) -> PathBuf {
        match folders.get(self.media_folder.saturating_sub(1)) {
            Some(f) if self.media_folder > 0 => paths::media_dir().join(f),
            _ => paths::media_dir(),
        }
    }

    /// The right-hand preview pane: image or video frame, plus play/seek for video and audio.
    fn media_preview(&mut self, ui: &mut Ui, height: f32) {
        let sel = self.media_sel.clone();
        let kind = sel.as_deref().and_then(media::kind_of);
        let controls_h = 30.0;
        let rect = egui::Rect::from_min_size(
            ui.next_widget_position(),
            egui::vec2(ui.available_width(), (height - controls_h).max(40.0)),
        );
        ui.allocate_rect(rect, egui::Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);
        let paint = |ui: &Ui, tex: &TextureHandle| {
            let r = crate::canvas::letterbox(rect, tex.size_vec2().x / tex.size_vec2().y.max(1.0));
            ui.painter().image(
                tex.id(),
                r,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        };
        match (sel.as_ref(), kind) {
            (Some(p), Some(media::Kind::Image)) => {
                let path = p.to_string_lossy().to_string();
                if let Some(tex) = self.texture(ui.ctx(), &path) {
                    paint(ui, &tex);
                }
            }
            (Some(_), Some(media::Kind::Video)) => {
                if let Some(tex) = self
                    .media
                    .texture(media::PREVIEW)
                    .or_else(|| self.media.still(media::PREVIEW))
                {
                    paint(ui, &tex);
                }
            }
            _ => {}
        }
        let st = self.media.status(media::PREVIEW);
        let playing = matches!(
            st.as_ref().map(|s| s.state),
            Some(deckcraft_media::PlayState::Playing | deckcraft_media::PlayState::Loading)
        );
        ui.horizontal(|ui| {
            ui.add_enabled_ui(
                matches!(kind, Some(media::Kind::Video | media::Kind::Audio)),
                |ui| {
                    if ui
                        .button(if playing { "\u{23f8}" } else { "\u{25b6}" })
                        .clicked()
                    {
                        self.media.toggle_pause(media::PREVIEW);
                    }
                },
            );
            if let Some(st) = &st {
                let mut pos = st.position as f32;
                let dur = st.duration.max(0.001) as f32;
                if ui
                    .add(egui::Slider::new(&mut pos, 0.0..=dur).show_value(false))
                    .changed()
                {
                    self.media.seek(media::PREVIEW, pos as f64);
                }
                ui.weak(format!(
                    "{} / {}",
                    media::fmt_time(st.position),
                    media::fmt_time(st.duration)
                ));
            }
        });
        ui.ctx().request_repaint();
    }

    /// New / Rename / Delete Folder windows for the media library.
    fn media_folder_windows(&mut self, ctx: &egui::Context) {
        if self.media_folder_dialog == 0 {
            return;
        }
        let kind = self.media_folder_dialog;
        let folders = Self::media_folders();
        let current = folders.get(self.media_folder.saturating_sub(1)).cloned();
        let mut done = false;
        egui::Window::new(match kind {
            1 => "New Folder",
            2 => "Rename Folder",
            _ => "Delete Folder",
        })
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            if kind == 3 {
                if let Some(name) = &current {
                    ui.label(format!(
                        "Delete the folder \"{name}\" and every file in it?"
                    ));
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.media_folder_name);
                });
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button("OK").clicked() {
                    let dir = paths::media_dir();
                    let name = self.media_folder_name.trim().to_string();
                    match kind {
                        1 => {
                            if !name.is_empty() {
                                match std::fs::create_dir(dir.join(&name)) {
                                    Ok(()) => {
                                        self.media_folder = Self::media_folders()
                                            .iter()
                                            .position(|f| f == &name)
                                            .map(|i| i + 1)
                                            .unwrap_or(0);
                                    }
                                    Err(_) => {
                                        self.status = "That folder already exists.".to_string()
                                    }
                                }
                            }
                            done = true;
                        }
                        2 => {
                            if !name.is_empty()
                                && let Some(cur) = &current
                                && cur != &name
                            {
                                if std::fs::rename(dir.join(cur), dir.join(&name)).is_ok() {
                                    self.media_folder = Self::media_folders()
                                        .iter()
                                        .position(|f| f == &name)
                                        .map(|i| i + 1)
                                        .unwrap_or(0);
                                }
                            }
                            done = true;
                        }
                        _ => {
                            if let Some(cur) = &current {
                                if std::fs::remove_dir_all(dir.join(cur)).is_ok() {
                                    self.media_folder = 0;
                                    self.media_sel = None;
                                    self.media.stop(media::PREVIEW);
                                } else {
                                    self.status =
                                        "Could not delete the folder. Some files may be in use."
                                            .to_string();
                                }
                            }
                            done = true;
                        }
                    }
                }
                if ui.button("Cancel").clicked() {
                    done = true;
                }
            });
        });
        if done {
            self.media_folder_dialog = 0;
        }
    }

    fn transport(&mut self, ui: &mut Ui) {
        let Some(st) = self.media.status(media::LIVE) else {
            return;
        };
        ui.horizontal(|ui| {
            let playing = matches!(
                st.state,
                deckcraft_media::PlayState::Playing | deckcraft_media::PlayState::Loading
            );
            if ui.button(if playing { "Pause" } else { "Play" }).clicked() {
                self.media.toggle_pause(media::LIVE);
            }
            if ui.button("Stop").clicked() {
                self.clear();
                return;
            }
            let mut pos = st.position;
            let dur = st.duration.max(0.0);
            ui.label(format!(
                "{} / {}",
                media::fmt_time(pos),
                media::fmt_time(dur)
            ));
            rtl_row(ui, |ui| {
                ui.checkbox(&mut self.media_loop, "Loop");
                ui.add_sized(
                    [110.0, ui.spacing().interact_size.y],
                    egui::Slider::new(&mut self.media_volume, 0.0..=1.0).show_value(false),
                )
                .on_hover_text("Volume");
                if dur > 0.0 {
                    ui.spacing_mut().slider_width = (ui.available_width() - 8.0).max(40.0);
                    if ui
                        .add(egui::Slider::new(&mut pos, 0.0..=dur).show_value(false))
                        .changed()
                    {
                        self.media.seek(media::LIVE, pos);
                    }
                }
            });
        });
        if let Some(e) = &st.error {
            ui.colored_label(Color32::LIGHT_RED, format!("Audio: {e}"));
        }
    }

    fn playlist_panel(&mut self, ui: &mut Ui) {
        const FILTERS: [&str; 8] = [
            "All",
            "Bible",
            "Songs",
            "Media",
            "Videos",
            "Photos",
            "YouTube",
            "Presentations",
        ];
        ui.label(
            RichText::new("Playlist / Order of Service")
                .strong()
                .size(15.0),
        );
        ui.horizontal(|ui| {
            ui.label("Filter:");
            egui::ComboBox::from_id_salt("pl_filter")
                .selected_text(FILTERS[self.playlist_filter.min(7)])
                .width(110.0)
                .show_ui(ui, |ui| {
                    for (i, f) in FILTERS.iter().enumerate() {
                        ui.selectable_value(&mut self.playlist_filter, i, *f);
                    }
                });
            if let Some(name) = self.playlist_path.as_ref().and_then(|p| p.file_stem()) {
                ui.weak(format!(
                    "{}{}",
                    name.to_string_lossy(),
                    if self.playlist_dirty { " *" } else { "" }
                ));
            } else if self.playlist_dirty {
                ui.weak("Unsaved *");
            }
        });
        let filter = self.playlist_filter;
        let list_h = ui.available_height() - button_row_height(ui);
        let mut activate = None;
        let mut select = None;
        list_box(ui, "playlist", list_h, |ui| {
            for (i, it) in self.playlist.items.iter().enumerate() {
                if !playlist_filter_matches(filter, it) {
                    continue;
                }
                let title = it.display_title();
                let video = media_item_is_video(it);
                let selected = self.playlist_sel == Some(i);
                let width = ui.available_width();
                let row_h = (ui.text_style_height(&egui::TextStyle::Body)
                    + ui.spacing().button_padding.y * 2.0)
                    .max(22.0);
                let (rect, resp) =
                    ui.allocate_exact_size(egui::vec2(width, row_h), egui::Sense::click());
                let visuals = ui.visuals().clone();
                let bg = if selected {
                    Some(visuals.selection.bg_fill)
                } else if resp.hovered() {
                    Some(visuals.widgets.hovered.weak_bg_fill)
                } else {
                    None
                };
                if let Some(bg) = bg {
                    ui.painter()
                        .rect_filled(rect, visuals.widgets.inactive.corner_radius, bg);
                }
                let icon_center = egui::Pos2::new(rect.left() + 13.0, rect.center().y);
                let icon_rect = egui::Rect::from_center_size(icon_center, egui::vec2(20.0, 20.0));
                paint_item_icon(
                    ui.painter(),
                    icon_rect,
                    it.kind,
                    video,
                    visuals.text_color(),
                );
                let text_w = (rect.width() - 32.0).max(10.0);
                let mut job = egui::text::LayoutJob::single_section(
                    title.clone(),
                    egui::TextFormat {
                        font_id: egui::TextStyle::Body.resolve(ui.style()),
                        color: visuals.text_color(),
                        ..Default::default()
                    },
                );
                job.wrap.max_width = text_w;
                job.wrap.max_rows = 1;
                job.wrap.break_anywhere = true;
                let galley = ui.painter().layout_job(job);
                let text_pos =
                    egui::Pos2::new(rect.left() + 28.0, rect.center().y - galley.size().y / 2.0);
                ui.painter().galley(text_pos, galley, visuals.text_color());
                if resp.double_clicked() {
                    activate = Some(i);
                } else if resp.clicked() {
                    select = Some(i);
                }
            }
            if self.playlist.items.is_empty() {
                ui.weak(
                    "Add Bible verses, songs, media or presentations with \"Add to Playlist\".",
                );
            }
        });
        if let Some(i) = select {
            self.playlist_sel = Some(i);
        }
        if let Some(i) = activate {
            self.activate_item(i);
        }
        let len = self.playlist.items.len();
        let sel = self.playlist_sel.filter(|&i| i < len);
        match (
            button_row(
                ui,
                &[
                    ("Remove", sel.is_some()),
                    ("Move Up", sel.is_some_and(|i| i > 0)),
                    ("Move Down", sel.is_some_and(|i| i + 1 < len)),
                ],
            ),
            sel,
        ) {
            (Some(0), Some(i)) => {
                self.playlist.items.remove(i);
                self.playlist_sel = None;
                self.playlist_dirty = true;
            }
            (Some(1), Some(i)) if i > 0 => {
                self.playlist.move_item(i, i - 1);
                self.playlist_sel = Some(i - 1);
                self.playlist_dirty = true;
            }
            (Some(2), Some(i)) if i + 1 < len => {
                self.playlist.move_item(i, i + 1);
                self.playlist_sel = Some(i + 1);
                self.playlist_dirty = true;
            }
            _ => {}
        }
    }

    fn side_panel(&mut self, ui: &mut Ui) {
        pill_tabs(
            ui,
            &mut self.side_tab,
            &[
                (SideTab::History, "History"),
                (SideTab::Notes, "Notes"),
                (SideTab::YouTube, "YouTube"),
                (SideTab::Google, "Google"),
            ],
        );
        ui.add_space(2.0);
        match self.side_tab {
            SideTab::History => self.history_view(ui),
            SideTab::Notes => {
                let live =
                    matches!(&self.content, Content::Lyrics { title, .. } if title == NOTES_TITLE);
                let mut show = live;
                ui.checkbox(&mut show, "Show notes on projection");
                let h = ui.available_height();
                let mut edited = false;
                list_box(ui, "notes", h, |ui| {
                    edited = ui
                        .add_sized(
                            ui.available_size(),
                            egui::TextEdit::multiline(&mut self.settings.notes)
                                .frame(egui::Frame::NONE),
                        )
                        .changed();
                });
                if show && (!live || edited) {
                    let text = self.settings.notes.clone();
                    self.set_live(
                        Content::Lyrics {
                            text,
                            title: NOTES_TITLE.into(),
                        },
                        Source::None,
                        "Notes".into(),
                    );
                } else if !show && live {
                    self.clear();
                }
            }
            SideTab::YouTube => self.web_tab(ui, true),
            SideTab::Google => self.web_tab(ui, false),
        }
    }

    fn history_view(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.menu_button("Types  ⏷", |ui| {
                for (i, t) in HISTORY_TYPES.iter().enumerate() {
                    if let Some(on) = self.history_types.get_mut(i) {
                        ui.checkbox(on, *t);
                    }
                }
            });
            let on = self.history_dups;
            let text = RichText::new("Show Duplicates");
            let b = if on {
                egui::Button::new(text.color(Color32::WHITE)).fill(Color32::from_rgb(22, 163, 74))
            } else {
                egui::Button::new(text)
            };
            if ui.add(b).clicked() {
                self.history_dups = !on;
            }
            if ui.button("Clear").clicked() {
                self.history.clear();
            }
        });
        let w = ui.available_width();
        let row_h = 20.0;
        let cols = [80.0, 80.0, (w * 0.2).max(110.0)];
        let col_rects = |r: egui::Rect| {
            let x0 = r.min.x;
            let x1 = x0 + cols[0];
            let x2 = x1 + cols[1];
            let x3 = x2 + cols[2];
            [(x0, x1), (x1, x2), (x2, x3), (x3, r.max.x)]
                .map(|(a, b)| egui::Rect::from_x_y_ranges(a..=b.max(a), r.y_range()))
        };
        let (head, _) = ui.allocate_exact_size(egui::vec2(w, row_h), egui::Sense::hover());
        let vis = ui.visuals().clone();
        let p = ui.painter();
        p.rect_filled(head, 0.0, vis.faint_bg_color);
        p.rect_stroke(
            head,
            0.0,
            vis.widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        for (r, t) in col_rects(head)
            .iter()
            .zip(["Time", "Type", "Title", "Content"])
        {
            p.text(
                r.center(),
                egui::Align2::CENTER_CENTER,
                t,
                egui::FontId::proportional(12.0),
                vis.text_color(),
            );
            p.vline(r.max.x, r.y_range(), vis.widgets.noninteractive.bg_stroke);
        }
        let mut go = None;
        let h = ui.available_height();
        let dups = self.history_dups;
        list_box(ui, "history", h, |ui| {
            let mut seen = std::collections::HashSet::new();
            for (i, e) in self.history.iter().enumerate() {
                let shown = HISTORY_TYPES
                    .iter()
                    .position(|t| *t == e.kind)
                    .and_then(|k| self.history_types.get(k).copied())
                    .unwrap_or(true);
                if !shown || (!dups && !seen.insert((e.kind, e.title.as_str()))) {
                    continue;
                }
                let (rect, resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), row_h),
                    egui::Sense::click(),
                );
                let p = ui.painter();
                if resp.hovered() {
                    p.rect_filled(rect, 0.0, vis.widgets.hovered.weak_bg_fill);
                } else if e.source != Source::None && e.source == self.source {
                    p.rect_filled(rect, 0.0, vis.selection.bg_fill);
                }
                for (r, t) in col_rects(rect).iter().zip([
                    e.time.as_str(),
                    e.kind,
                    e.title.as_str(),
                    e.content.as_str(),
                ]) {
                    let line = t.lines().next().unwrap_or_default();
                    p.with_clip_rect(r.shrink2(egui::vec2(3.0, 0.0))).text(
                        egui::pos2(r.min.x + 4.0, r.center().y),
                        egui::Align2::LEFT_CENTER,
                        line,
                        egui::FontId::proportional(12.0),
                        vis.text_color(),
                    );
                }
                if resp.clicked() {
                    go = Some(i);
                }
            }
        });
        if let Some(i) = go
            && let Some(e) = self.history.get(i)
        {
            match (e.replay.clone(), e.source.clone()) {
                (Some(item), _) => self.project_item(item),
                (None, src) => self.project_source(&src),
            }
        }
    }

    fn preview_panel(&mut self, ui: &mut Ui) {
        ui.label("Projection Preview");
        let has_media = self.media.status(media::LIVE).is_some();
        let ctl_h = if has_media {
            ui.spacing().interact_size.y + 10.0
        } else {
            0.0
        };
        let avail = ui.available_rect_before_wrap().intersect(ui.clip_rect());
        let group = egui::Rect::from_min_size(
            avail.min,
            egui::vec2(avail.width(), (avail.height() - ctl_h).max(40.0)),
        );
        let aspect =
            self.settings.output_size[0] as f32 / self.settings.output_size[1].max(1) as f32;
        let inner = group.shrink(4.0);
        let w = inner.width().min(inner.height() * aspect).max(1.0);
        let rect = egui::Rect::from_min_size(
            egui::pos2(inner.center().x - w / 2.0, inner.min.y),
            egui::vec2(w, w / aspect),
        );
        self.paint_live(ui, rect);
        if self.content == Content::Empty && !self.black {
            ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
            let logo = paths::resource_dir()
                .join("SPlogo.png")
                .to_string_lossy()
                .into_owned();
            if let Some(t) = self.texture(ui.ctx(), &logo) {
                let s = t.size_vec2();
                let k = (rect.width() * 0.5 / s.x).min(rect.height() * 0.6 / s.y);
                let r = egui::Rect::from_center_size(rect.center(), s * k);
                ui.painter().image(
                    t.id(),
                    r,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    Color32::WHITE,
                );
            }
        }
        ui.painter().rect_stroke(
            group,
            2.0,
            ui.visuals().widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        ui.allocate_rect(group, egui::Sense::hover());
        if has_media {
            self.transport(ui);
        }
    }

    fn projector(&mut self, ctx: &egui::Context) {
        if !self.projector_open {
            self.projector_pos = None;
            return;
        }
        let mut close = false;
        let mut toggle_fs = false;
        let pos = match self.projector_pos {
            Some(p) => p,
            None => {
                // Like the original app, the projection always opens fullscreen
                // on the selected display (F11 / double-click / Esc toggles).
                self.projector_fullscreen = true;
                let d = self.displays.get(self.settings.projector_monitor);
                let p = d
                    .map(|d| [d.pos[0] + 40.0, d.pos[1] + 40.0])
                    .unwrap_or([80.0, 80.0]);
                self.projector_pos = Some(p);
                p
            }
        };
        ctx.show_viewport_immediate(
            projector_id(),
            ViewportBuilder::default()
                .with_title("SimplePresenter — Projection")
                .with_inner_size([960.0, 540.0])
                .with_position(pos)
                .with_fullscreen(self.projector_fullscreen),
            |ui, _class| {
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE.fill(Color32::BLACK))
                    .show(ui, |ui| {
                        let rect = ui.max_rect();
                        self.paint_live(ui, rect);
                        let r = ui.allocate_rect(rect, egui::Sense::click());
                        if r.double_clicked() {
                            toggle_fs = true;
                        }
                    });
                if ui.input(|i| i.viewport().close_requested()) {
                    close = true;
                }
                let (esc, f11, next, prev) = ui.input(|i| {
                    (
                        i.key_pressed(Key::Escape),
                        i.key_pressed(Key::F11),
                        i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::Space),
                        i.key_pressed(Key::ArrowLeft),
                    )
                });
                if f11 {
                    toggle_fs = true;
                }
                if esc && self.projector_fullscreen {
                    toggle_fs = true;
                }
                if next {
                    self.step(true);
                }
                if prev {
                    self.step(false);
                }
            },
        );
        if toggle_fs {
            self.projector_fullscreen = !self.projector_fullscreen;
            ctx.send_viewport_cmd_to(
                projector_id(),
                ViewportCommand::Fullscreen(self.projector_fullscreen),
            );
        }
        if close {
            self.projector_open = false;
            self.projector_fullscreen = false;
            self.projector_pos = None;
        }
    }

    fn update_window(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.update_rx
            && let Ok(result) = rx.try_recv()
        {
            self.update_rx = None;
            match result {
                Ok(Some(r)) => self.update_found = Some(r),
                Ok(None) if self.update_manual => {
                    self.status =
                        format!("SimplePresenter {} is up to date.", crate::update::VERSION)
                }
                Err(e) if self.update_manual => self.status = format!("Update check failed: {e}"),
                _ => {}
            }
            self.update_manual = false;
        }
        let Some(r) = self.update_found.clone() else {
            return;
        };
        let mut open = true;
        let mut close = false;
        egui::Window::new("Update Available")
            .open(&mut open)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "SimplePresenter {} is available (you have {}).",
                    r.version,
                    crate::update::VERSION
                ));
                if !r.notes.is_empty() {
                    ui.separator();
                    ui.label(&r.notes);
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if !r.download_url.is_empty() && ui.button("Download").clicked() {
                        ctx.open_url(egui::OpenUrl::new_tab(&r.download_url));
                        close = true;
                    }
                    if ui.button("Later").clicked() {
                        close = true;
                    }
                });
            });
        if !open || close {
            self.update_found = None;
        }
    }

    /// Saves settings changed outside the Settings dialog (Bible choice, notes, etc.)
    /// so they survive a crash or forced close; the dialog itself saves on OK.
    fn autosave(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        if self.settings_open || now < self.autosave_at {
            return;
        }
        self.autosave_at = now + 2.0;
        let Ok(text) = serde_json::to_string(&self.settings) else {
            return;
        };
        if self.saved_settings.is_empty() {
            self.saved_settings = text;
        } else if text != self.saved_settings {
            match self.settings.save() {
                Ok(()) => self.saved_settings = text,
                Err(e) => self.status = e,
            }
        }
    }

    fn settings_window(&mut self, ctx: &egui::Context) {
        use crate::settings_ui as sui;
        if !self.settings_open {
            self.settings_backup = None;
            self.style_dialog = 0;
            return;
        }
        if self.settings_backup.is_none() {
            self.settings_backup = Some(self.settings.clone());
            self.displays = crate::displays::list();
        }
        let mut open = true;
        let mut accept = false;
        let mut cancel = false;
        let mut theme_changed = false;
        let mut restart_server = false;
        let accent = crate::theme::accent(self.settings.dark_mode);
        egui::Window::new("Settings").open(&mut open).default_size([640.0, 580.0]).collapsible(false).show(ctx, |ui| {
            pill_tabs(ui, &mut self.settings_tab, &[(0, "Projection"), (1, "Backgrounds"), (2, "OBS Overlay"), (3, "General")]);
            ui.separator();
            let h = (ui.ctx().content_rect().height() - 220.0).clamp(240.0, 470.0);
            egui::ScrollArea::vertical().max_height(h).min_scrolled_height(h).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                match self.settings_tab {
                    0 => {
                        sui::group(ui, "Display Selection", |ui| {
                            sui::form(ui, "display", |f| {
                                f.row("Projection Display:", |ui| {
                                    let labels: Vec<String> = if self.displays.is_empty() { vec!["Display 1".into()] } else { self.displays.iter().map(|d| d.label.clone()).collect() };
                                    let cur = self.settings.projector_monitor.min(labels.len().saturating_sub(1));
                                    egui::ComboBox::from_id_salt("display-combo").selected_text(labels.get(cur).cloned().unwrap_or_default()).width(300.0).show_ui(ui, |ui| {
                                        for (i, l) in labels.iter().enumerate() {
                                            ui.selectable_value(&mut self.settings.projector_monitor, i, l);
                                        }
                                    });
                                });
                                f.note("Select which monitor/display to show the projection canvas in fullscreen.");
                            });
                        });
                        if sui::launcher(ui, "Song Lyrics", "Song Lyrics Settings...", "Configure fonts, colors, and highlight options for song lyrics.") {
                            self.style_dialog = 1;
                        }
                        if sui::launcher(ui, "Bible Text", "Bible Text Settings...", "Configure fonts, colors, and highlight options for Bible verses.") {
                            self.style_dialog = 2;
                        }
                        if sui::launcher(ui, "Bible Reference Format", "Bible Reference Settings...", "Import Bibles, choose the default version, and style the verse reference.") {
                            self.style_dialog = 3;
                        }
                    }
                    1 => {
                        let s = &mut self.settings;
                        ui.checkbox(&mut s.separate_backgrounds, "Use separate backgrounds");
                        ui.checkbox(&mut s.pause_video_in_preview, "Pause video backgrounds in the preview pane\n(shows a still frame — saves resources; projection output is unaffected)");
                        ui.checkbox(&mut s.show_background_in_preview, "Show backgrounds in the preview pane\n(uncheck to hide them entirely — saves resources; projection output is unaffected)");
                        let sep = s.separate_backgrounds;
                        sui::background(ui, "Bible Verses Background", &mut s.bible_overlay.background, "bg-bible", true);
                        if !sep {
                            s.song_overlay.background = s.bible_overlay.background.clone();
                        }
                        sui::background(ui, "Song Lyrics Background", &mut s.song_overlay.background, "bg-song", sep);
                        sui::background(ui, "Notes Background", &mut s.notes_background, "bg-notes", sep);
                    }
                    2 => {
                        let ip = remote::lan_ip();
                        let port = self.settings.overlay_server_port;
                        let mut copy = |ui: &mut Ui, label: &str, path: &str| {
                            let url = format!("http://{ip}:{port}{path}");
                            ui.add(egui::TextEdit::singleline(&mut url.as_str()).desired_width(280.0).font(egui::TextStyle::Monospace));
                            if ui.button("Copy URL").clicked() {
                                ui.ctx().copy_text(url);
                                self.status = format!("Copied {label}");
                            }
                        };
                        sui::group(ui, "OBS Browser Source URL", |ui| {
                            sui::form(ui, "obs-urls", |f| {
                                f.row("Overlay Server:", |ui| {
                                    if ui.checkbox(&mut self.settings.overlay_server_enabled, "Enabled").changed() {
                                        restart_server = true;
                                    }
                                });
                                f.row("Network address:", |ui| ui.monospace(ip.to_string()));
                                f.row("Landscape URL:", |ui| copy(ui, "Landscape URL", "/overlay"));
                                f.row("Portrait URL:", |ui| copy(ui, "Portrait URL", "/overlay?portrait"));
                                f.row("Remote URL:", |ui| copy(ui, "Remote URL", "/remote"));
                            });
                            if !self.server_error.is_empty() {
                                ui.colored_label(Color32::LIGHT_RED, &self.server_error);
                            }
                        });
                        sui::group(ui, "Projection Stream URL", |ui| {
                            sui::form(ui, "obs-stream", |f| {
                                f.row("Stream URL:", |ui| copy(ui, "Stream URL", "/projection.mjpg"));
                            });
                        });
                        sui::note(ui, &format!("If other devices cannot connect, allow SimplePresenter in the firewall (ports {port} and {}).", port.saturating_add(1)));
                        sui::group(ui, "Canvas Resolution", |ui| {
                            sui::form(ui, "obs-res", |f| {
                                f.row("Resolution:", |ui| {
                                    let sizes: [([u32; 2], &str); 4] = [([1920, 1080], "1920x1080 (Full HD)"), ([1280, 720], "1280x720 (HD)"), ([3840, 2160], "3840x2160 (4K)"), ([2560, 1440], "2560x1440 (2K)")];
                                    let cur = self.settings.obs_canvas;
                                    let text = sizes.iter().find(|(s, _)| *s == cur).map(|(_, l)| l.to_string()).unwrap_or_else(|| format!("{}x{} (Custom)", cur[0], cur[1]));
                                    egui::ComboBox::from_id_salt("obs-res-combo").selected_text(text).width(200.0).show_ui(ui, |ui| {
                                        for (s, l) in sizes {
                                            ui.selectable_value(&mut self.settings.obs_canvas, s, l);
                                        }
                                    });
                                });
                                f.note("Note: Changing resolution requires refreshing the OBS Browser Source.");
                            });
                        });
                        if sui::launcher(ui, "Text Formatting", "Text Formatting Settings...", "Configure fonts, colors, shadows, outlines, and highlights for text.") {
                            self.style_dialog = 4;
                        }
                        if sui::launcher(ui, "Bible Reference Formatting", "Bible Reference Formatting Settings...", "Configure fonts, colors, shadows, outlines, and highlights for Bible references.") {
                            self.style_dialog = 5;
                        }
                    }
                    _ => {
                        let s = &mut self.settings;
                        sui::group(ui, "Appearance", |ui| {
                            sui::form(ui, "gen-look", |f| {
                                f.row("Theme:", |ui| {
                                    if ui.checkbox(&mut s.dark_mode, "Dark theme").changed() {
                                        theme_changed = true;
                                    }
                                });
                                f.row("Interface Size:", |ui| {
                                    if ui.add(egui::Slider::new(&mut s.ui_scale, 0.75..=2.0).step_by(0.05)).changed() {
                                        theme_changed = true;
                                    }
                                });
                            });
                        });
                        sui::group(ui, "Slides", |ui| {
                            sui::form(ui, "gen-slides", |f| {
                                f.row("Bible verses per slide:", |ui| ui.add(egui::DragValue::new(&mut s.bible_verses_per_slide).range(1..=10)));
                                f.row("Custom output size:", |ui| {
                                    ui.add(egui::DragValue::new(&mut s.output_size[0]).range(320..=7680));
                                    ui.label("×");
                                    ui.add(egui::DragValue::new(&mut s.output_size[1]).range(240..=4320));
                                });
                            });
                        });
                        sui::group(ui, "Overlay Server", |ui| {
                            sui::form(ui, "gen-server", |f| {
                                f.row("Port:", |ui| {
                                    ui.add(egui::DragValue::new(&mut s.overlay_server_port).range(1024..=65534));
                                    if ui.button("Restart server").clicked() {
                                        restart_server = true;
                                    }
                                });
                            });
                        });
                        sui::group(ui, "Updates", |ui| {
                            ui.checkbox(&mut s.check_updates, "Check for updates on start");
                        });
                        sui::group(ui, "Data Folder", |ui| {
                            ui.horizontal(|ui| {
                                ui.monospace(paths::data_dir().display().to_string());
                                if ui.button("Copy").clicked() {
                                    ui.ctx().copy_text(paths::data_dir().display().to_string());
                                }
                            });
                        });
                    }
                }
            });
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new("Cancel").min_size(egui::vec2(84.0, 28.0))).clicked() {
                    cancel = true;
                }
                if ui.add(egui::Button::new(RichText::new("OK").color(Color32::WHITE)).fill(accent).min_size(egui::vec2(84.0, 28.0))).clicked() {
                    accept = true;
                }
            });
        });
        self.style_dialog_window(ctx);
        if !self.settings.separate_backgrounds {
            self.settings.song_overlay.background = self.settings.bible_overlay.background.clone();
        }
        if cancel {
            if let Some(old) = self.settings_backup.take() {
                let server_changed = old.overlay_server_enabled
                    != self.settings.overlay_server_enabled
                    || old.overlay_server_port != self.settings.overlay_server_port;
                let bible_changed = old.last_bible != self.settings.last_bible;
                let notes = std::mem::take(&mut self.settings.notes);
                self.settings = old;
                self.settings.notes = notes;
                restart_server |= server_changed;
                theme_changed = true;
                if bible_changed {
                    let idx = self
                        .bibles
                        .iter()
                        .position(|p| p.to_string_lossy() == self.settings.last_bible)
                        .unwrap_or(0);
                    self.load_bible(idx);
                }
            }
            self.settings_open = false;
        }
        if restart_server {
            self.start_server();
        }
        if theme_changed {
            crate::theme::apply(ctx, self.settings.dark_mode);
            ctx.set_zoom_factor(self.settings.ui_scale.clamp(0.75, 2.0));
        }
        if accept || !open {
            self.settings_open = false;
            if let Err(e) = self.settings.save() {
                self.status = e;
            }
        }
        if !self.settings_open {
            self.settings_backup = None;
            self.style_dialog = 0;
        }
    }

    fn style_dialog_window(&mut self, ctx: &egui::Context) {
        use crate::settings_ui as sui;
        let title = match self.style_dialog {
            1 => "Song Lyrics Settings",
            2 => "Bible Text Settings",
            3 => "Bible Reference Format",
            4 => "OBS Text Formatting",
            5 => "OBS Bible Reference Formatting",
            _ => return,
        };
        let mut open = true;
        let mut done = false;
        let mut pick_bible = None;
        let mut import = false;
        let mut delete: Option<PathBuf> = None;
        let accent = crate::theme::accent(self.settings.dark_mode);
        egui::Window::new(title).id(egui::Id::new("style_dialog")).open(&mut open).default_size([560.0, 620.0]).collapsible(false).show(ctx, |ui| {
            let h = (ui.ctx().content_rect().height() - 220.0).clamp(240.0, 560.0);
            egui::ScrollArea::vertical().max_height(h).min_scrolled_height(h).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                match self.style_dialog {
                    1 => sui::song_lyrics(ui, &mut self.settings.song_overlay, &self.fonts.families),
                    2 => sui::bible_text(ui, &mut self.settings.bible_overlay, &self.fonts.families),
                    4 => sui::obs_text(ui, &mut self.settings.obs_overlay, &self.fonts.families),
                    5 => sui::obs_reference(ui, &mut self.settings.obs_overlay, &self.fonts.families),
                    _ => {
                        if ui.button("Import Bible XML...").clicked() {
                            import = true;
                        }
                        sui::note(ui, "Import a Bible in XML format. Imported Bibles are copied to the app's Bible folder and become available in the Default Bible Version list.");
                        sui::group(ui, "Translation", |ui| {
                            sui::form(ui, "ref-version", |f| {
                                f.row("Default Bible Version:", |ui| {
                                    let name = |p: &PathBuf| p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| p.to_string_lossy().into_owned());
                                    let def = self.settings.default_bible.clone();
                                    let cur = self.bibles.iter().find(|p| !def.is_empty() && p.to_string_lossy() == def).map(name).unwrap_or_else(|| "Automatic (first available)".into());
                                    ui.add_enabled_ui(!self.bibles.is_empty(), |ui| {
                                        egui::ComboBox::from_id_salt("ref-version-combo").selected_text(cur).width(220.0).show_ui(ui, |ui| {
                                            if ui.selectable_label(def.is_empty(), "Automatic (first available)").clicked() {
                                                pick_bible = Some(None);
                                            }
                                            for (i, p) in self.bibles.iter().enumerate() {
                                                if ui.selectable_label(p.to_string_lossy() == def, name(p)).clicked() {
                                                    pick_bible = Some(Some(i));
                                                }
                                            }
                                        });
                                    });
                                    let path = self.bibles.iter().find(|p| !def.is_empty() && p.to_string_lossy() == def).cloned();
                                    let can = path.as_deref().is_some_and(bible::can_delete);
                                    if ui.add_enabled(can, egui::Button::new("Delete...")).clicked() {
                                        delete = path;
                                    }
                                });
                            });
                        });
                        sui::bible_reference(ui, &mut self.settings.bible_overlay, &self.fonts.families);
                    }
                }
            });
            ui.separator();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(RichText::new("OK").color(Color32::WHITE)).fill(accent).min_size(egui::vec2(84.0, 28.0))).clicked() {
                    done = true;
                }
                if ui.button("Reset to Defaults").clicked() {
                    let d = Settings::default();
                    match self.style_dialog {
                        1 => {
                            let bg = self.settings.song_overlay.background.clone();
                            self.settings.song_overlay = OverlayConfig { background: bg, ..d.song_overlay };
                        }
                        2 => {
                            let o = &mut self.settings.bible_overlay;
                            let dd = d.bible_overlay;
                            o.text = dd.text;
                            o.vertical = dd.vertical;
                            o.padding = dd.padding;
                            o.line_spacing = dd.line_spacing;
                        }
                        4 => {
                            let o = &mut self.settings.obs_overlay;
                            o.text = d.obs_overlay.text;
                        }
                        5 => {
                            let o = &mut self.settings.obs_overlay;
                            let dd = d.obs_overlay;
                            o.reference = dd.reference;
                            o.reference_pos = dd.reference_pos;
                            o.reference_spacing = dd.reference_spacing;
                            o.display_version = dd.display_version;
                        }
                        _ => {
                            let o = &mut self.settings.bible_overlay;
                            let dd = d.bible_overlay;
                            o.reference = dd.reference;
                            o.show_reference = dd.show_reference;
                            o.reference_pos = dd.reference_pos;
                            o.separate_reference_area = dd.separate_reference_area;
                            o.display_version = dd.display_version;
                        }
                    }
                }
            });
        });
        if let Some(pick) = pick_bible {
            self.settings.default_bible = pick
                .and_then(|i| self.bibles.get(i))
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.load_bible(pick.unwrap_or(0));
        }
        if let Some(path) = delete {
            self.delete_bible(&path);
        }
        if import {
            self.import_bible();
        }
        if done || !open {
            self.style_dialog = 0;
        }
    }

    fn delete_bible(&mut self, path: &std::path::Path) {
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let yes = rfd::MessageDialog::new()
            .set_title("Delete Bible")
            .set_description(format!(
                "Delete the Bible \"{name}\"?\n\nThis removes the file from the Bible folder."
            ))
            .set_buttons(rfd::MessageButtons::YesNo)
            .show();
        if yes != rfd::MessageDialogResult::Yes {
            return;
        }
        if let Err(e) = std::fs::remove_file(path) {
            self.status = format!("Could not delete Bible: {e}");
            return;
        }
        self.bibles = bible::available_bibles();
        self.settings.default_bible.clear();
        self.load_bible(0);
        self.status = format!("Deleted {name}");
    }

    fn import_bible(&mut self) {
        let Some(src) = rfd::FileDialog::new()
            .add_filter("Bible XML", &["xml"])
            .pick_file()
        else {
            return;
        };
        if let Err(e) = Bible::load(&src) {
            self.status = format!("Not a Bible XML file: {e}");
            return;
        }
        let Some(name) = src.file_name() else {
            return;
        };
        let dest = bible::bible_directory().join(name);
        match std::fs::copy(&src, &dest) {
            Ok(_) => {
                self.bibles = bible::available_bibles();
                self.status = format!("Imported {}", name.to_string_lossy());
            }
            Err(e) => self.status = format!("Could not import Bible: {e}"),
        }
    }
}

const NOTES_TITLE: &str = "\u{1}notes";

/// A free name for `src` in `dir`, as the original: `name.ext`, then `name_1.ext`, `name_2.ext`...
fn unique_path(dir: &std::path::Path, src: &std::path::Path) -> Option<PathBuf> {
    let name = src.file_name()?;
    let first = dir.join(name);
    if !first.exists() {
        return Some(first);
    }
    let stem = src.file_stem()?.to_string_lossy();
    let ext = src
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    (1..100_000u32)
        .map(|n| dir.join(format!("{stem}_{n}{ext}")))
        .find(|p| !p.exists())
}

fn projector_id() -> ViewportId {
    ViewportId::from_hash_of("simplepresenter_projector")
}

fn superscript(n: u32) -> String {
    const D: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    n.to_string()
        .chars()
        .filter_map(|c| c.to_digit(10).and_then(|d| D.get(d as usize).copied()))
        .collect()
}

fn open_folder(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(not(any(windows, target_os = "macos")))]
    let cmd = "xdg-open";
    let _ = std::process::Command::new(cmd).arg(path).spawn();
}

/// A one-line row laid out right to left (buttons first, then a field filling the rest).
fn rtl_row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), add)
}

/// Rounded tab buttons like the Qt app's tab bars (black when selected).
fn pill_tabs<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, items: &[(T, &str)]) {
    let dark = ui.visuals().dark_mode;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.spacing_mut().button_padding = egui::vec2(14.0, 4.0);
        for (v, label) in items {
            let sel = *current == *v;
            let (fill, fg) = match (sel, dark) {
                (true, _) => (crate::theme::accent(dark), Color32::WHITE),
                (false, false) => (Color32::WHITE, Color32::from_gray(70)),
                (false, true) => (Color32::from_rgb(42, 46, 55), Color32::from_gray(210)),
            };
            let stroke = if sel {
                egui::Stroke::NONE
            } else {
                ui.visuals().widgets.noninteractive.bg_stroke
            };
            let b = egui::Button::new(RichText::new(*label).color(fg).size(13.0))
                .fill(fill)
                .stroke(stroke)
                .corner_radius(14.0)
                .min_size(egui::vec2(60.0, 28.0));
            if ui.add(b).clicked() {
                *current = *v;
            }
        }
    });
}

/// A selectable, left-aligned, wrapping list row filling the width.
fn list_row(ui: &mut Ui, selected: bool, text: RichText) -> egui::Response {
    let w = ui.available_width().max(20.0);
    let pad = egui::vec2(4.0, 2.0);
    let galley = egui::WidgetText::from(text).into_galley(
        ui,
        Some(egui::TextWrapMode::Wrap),
        w - pad.x * 2.0,
        egui::TextStyle::Button,
    );
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(w, galley.size().y + pad.y * 2.0),
        egui::Sense::click(),
    );
    let vis = ui.visuals();
    if selected {
        ui.painter().rect_filled(rect, 2.0, vis.selection.bg_fill);
    } else if resp.hovered() {
        ui.painter()
            .rect_filled(rect, 2.0, vis.widgets.hovered.weak_bg_fill);
    }
    let color = if selected {
        vis.selection.stroke.color
    } else {
        vis.text_color()
    };
    ui.painter().galley(rect.min + pad, galley, color);
    resp
}

fn button_row_height(ui: &Ui) -> f32 {
    ui.spacing().interact_size.y + 4.0 + ui.spacing().item_spacing.y * 2.0
}

/// A row of equal-width buttons filling the width; returns the index clicked.
fn button_row(ui: &mut Ui, items: &[(&str, bool)]) -> Option<usize> {
    let n = items.len().max(1) as f32;
    let sp = ui.spacing().item_spacing.x;
    let w = ((ui.available_width() - sp * (n - 1.0)) / n).max(30.0);
    let h = ui.spacing().interact_size.y + 4.0;
    let mut hit = None;
    ui.horizontal(|ui| {
        for (i, (label, enabled)) in items.iter().enumerate() {
            if ui
                .add_enabled(
                    *enabled,
                    egui::Button::new(*label).min_size(egui::vec2(w, h)),
                )
                .clicked()
            {
                hit = Some(i);
            }
        }
    });
    hit
}

/// A bordered, scrolling list area of a fixed height, like a Qt list widget.
fn list_box<R>(ui: &mut Ui, id: &str, height: f32, add: impl FnOnce(&mut Ui) -> R) -> R {
    let w = (ui.available_width() - 6.0).max(20.0);
    let h = (height - 8.0).max(24.0);
    egui::Frame::new()
        .fill(ui.visuals().extreme_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .inner_margin(egui::Margin::same(2))
        .show(ui, |ui| {
            ui.set_min_size(egui::vec2(w, h));
            ui.set_max_width(w);
            egui::ScrollArea::vertical()
                .id_salt(id)
                .auto_shrink([false, false])
                .max_height(h)
                .show(ui, add)
                .inner
        })
        .inner
}

/// A media file's library name: the file name without its extension, like the original.
fn media_display_name(p: &std::path::Path) -> String {
    p.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .or_else(|| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_default()
}

/// Whether a playlist media item is a video (for its icon), from its stored path.
fn media_item_is_video(it: &Item) -> bool {
    matches!(
        media::kind_of(std::path::Path::new(
            it.data
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(&it.reference),
        )),
        Some(media::Kind::Video)
    )
}

/// Small type icon for playlist rows, drawn like the original's icons.
fn paint_item_icon(p: &egui::Painter, rect: egui::Rect, kind: ItemType, video: bool, fg: Color32) {
    use egui::{CornerRadius, Pos2, Shape, Stroke, StrokeKind};
    let stroke = Stroke::new(1.4, fg);
    let r = rect.shrink(3.0);
    let w = r.width();
    let h = r.height();
    match kind {
        ItemType::Bible => {
            let mid = Pos2::new(r.center().x, r.center().y);
            let left = egui::Rect::from_min_max(r.min, mid);
            let right = egui::Rect::from_min_max(mid, r.max);
            p.rect_stroke(left, 0.0, stroke, StrokeKind::Inside);
            p.rect_stroke(right, 0.0, stroke, StrokeKind::Inside);
        }
        ItemType::Song => {
            let head = Pos2::new(r.left() + w * 0.3, r.bottom() - h * 0.28);
            let rad = (h * 0.15).max(1.5);
            p.circle_filled(head, rad, fg);
            p.line_segment(
                [
                    Pos2::new(head.x + rad, head.y),
                    Pos2::new(head.x + rad, r.top() + h * 0.12),
                ],
                stroke,
            );
            p.line_segment(
                [
                    Pos2::new(head.x + rad, r.top() + h * 0.12),
                    Pos2::new(r.right() - 1.0, r.top() + h * 0.34),
                ],
                stroke,
            );
        }
        ItemType::Media => {
            if video {
                p.rect_stroke(r, CornerRadius::same(2), stroke, StrokeKind::Inside);
                let tri = vec![
                    Pos2::new(r.left() + w * 0.36, r.top() + h * 0.32),
                    Pos2::new(r.left() + w * 0.36, r.bottom() - h * 0.32),
                    Pos2::new(r.right() - w * 0.12, r.center().y),
                ];
                p.add(Shape::convex_polygon(tri, fg, Stroke::NONE));
            } else {
                p.rect_stroke(r, CornerRadius::same(2), stroke, StrokeKind::Inside);
                p.line_segment(
                    [
                        Pos2::new(r.left() + 1.5, r.bottom() - 1.5),
                        Pos2::new(r.left() + w * 0.42, r.top() + h * 0.34),
                    ],
                    stroke,
                );
                p.line_segment(
                    [
                        Pos2::new(r.left() + w * 0.42, r.top() + h * 0.34),
                        Pos2::new(r.right() - 1.5, r.bottom() - 1.5),
                    ],
                    stroke,
                );
                p.circle_filled(Pos2::new(r.right() - w * 0.28, r.top() + h * 0.3), 1.6, fg);
            }
        }
        ItemType::YouTube => {
            let red = Color32::from_rgb(204, 32, 36);
            p.rect_filled(r, CornerRadius::same(4), red);
            let tri = vec![
                Pos2::new(r.left() + w * 0.36, r.top() + h * 0.28),
                Pos2::new(r.left() + w * 0.36, r.bottom() - h * 0.28),
                Pos2::new(r.right() - w * 0.16, r.center().y),
            ];
            p.add(Shape::convex_polygon(tri, Color32::WHITE, Stroke::NONE));
        }
        ItemType::PowerPoint => {
            let screen =
                egui::Rect::from_min_max(r.min, Pos2::new(r.right(), r.bottom() - h * 0.28));
            p.rect_stroke(screen, CornerRadius::same(2), stroke, StrokeKind::Inside);
            p.line_segment(
                [
                    Pos2::new(r.center().x - w * 0.18, r.bottom() - 1.0),
                    Pos2::new(r.center().x + w * 0.18, r.bottom() - 1.0),
                ],
                stroke,
            );
            let top = screen.top() + 2.0;
            p.line_segment(
                [
                    Pos2::new(screen.left() + 2.0, top + h * 0.1),
                    Pos2::new(screen.right() - 4.0, top + h * 0.1),
                ],
                stroke,
            );
            p.line_segment(
                [
                    Pos2::new(screen.left() + 2.0, top + h * 0.22),
                    Pos2::new(screen.right() - 7.0, top + h * 0.22),
                ],
                stroke,
            );
        }
    }
}

fn playlist_filter_matches(filter: usize, it: &Item) -> bool {
    let media_kind = || {
        media::kind_of(std::path::Path::new(
            it.data
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or(&it.reference),
        ))
    };
    match filter {
        1 => matches!(it.kind, ItemType::Bible),
        2 => matches!(it.kind, ItemType::Song),
        3 => matches!(it.kind, ItemType::Media),
        4 => matches!(it.kind, ItemType::Media) && media_kind() == Some(media::Kind::Video),
        5 => matches!(it.kind, ItemType::Media) && media_kind() == Some(media::Kind::Image),
        6 => matches!(it.kind, ItemType::YouTube),
        7 => matches!(it.kind, ItemType::PowerPoint),
        _ => true,
    }
}

/// A single-line text field filling the rest of the row (exactly, so side panels don't grow).
fn fill_text(ui: &mut Ui, text: &mut String, hint: &str) -> egui::Response {
    let h = ui.spacing().interact_size.y;
    ui.add_sized(
        [ui.available_width().max(40.0), h],
        egui::TextEdit::singleline(text).hint_text(hint),
    )
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.shortcuts(ctx);
        self.media.tick(ctx);
        self.poll_deck(ctx);
        self.autosave(ctx);
        for fam in [
            &self.settings.bible_overlay.text,
            &self.settings.bible_overlay.reference,
            &self.settings.song_overlay.text,
            &self.settings.song_overlay.reference,
            &self.settings.obs_overlay.text,
            &self.settings.obs_overlay.reference,
        ] {
            self.fonts.ensure(ctx, &fam.family);
        }
        let want = (self.projector_open, self.projector_fullscreen);
        if matches!(self.content, Content::YouTube { .. }) && want != self.player_for {
            self.restart_player();
        }
        self.server_frame(ctx);
    }

    fn ui(&mut self, ui: &mut Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.web_rect = None;
        self.capture(&ctx, frame);
        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(&self.status);
                rtl_row(ui, |ui| {
                    let live = match &self.source {
                        Source::None => "—".to_string(),
                        Source::Bible {
                            book,
                            chapter,
                            start,
                            ..
                        } => format!("{book} {chapter}:{start}"),
                        Source::Song { title, section } => format!("{title} #{}", section + 1),
                        Source::Slide { path, index } => {
                            format!("{} slide {}", crate::ppt::name_of(path), index + 1)
                        }
                    };
                    ui.label(format!("Live: {live}"));
                });
            });
        });
        let upper_h = ui.available_height() * 0.6;
        egui::Panel::top("main_upper")
            .resizable(true)
            .default_size(upper_h)
            .min_size(220.0)
            .show(ui, |ui| {
                let w = ui.available_width();
                egui::Panel::right("main_playlist")
                    .resizable(true)
                    .default_size(w / 3.0)
                    .min_size(240.0)
                    .show(ui, |ui| self.playlist_panel(ui));
                egui::CentralPanel::default().show(ui, |ui| {
                    pill_tabs(
                        ui,
                        &mut self.tab,
                        &[
                            (Tab::Bible, "Bible"),
                            (Tab::Songs, "Songs"),
                            (Tab::Media, "Media"),
                            (Tab::PowerPoint, "PowerPoint"),
                        ],
                    );
                    ui.add_space(2.0);
                    match self.tab {
                        Tab::Bible => self.bible_tab(ui),
                        Tab::Songs => self.songs_tab(ui),
                        Tab::Media => self.media_tab(ui),
                        Tab::PowerPoint => self.ppt_tab(ui),
                    }
                });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            let w = ui.available_width();
            egui::Panel::left("main_preview")
                .resizable(true)
                .default_size(w / 2.0)
                .min_size(300.0)
                .show(ui, |ui| self.preview_panel(ui));
            egui::CentralPanel::default().show(ui, |ui| self.side_panel(ui));
        });
        self.update_webviews(&ctx, frame);
        self.projector(&ctx);
        self.settings_window(&ctx);
        self.media_folder_windows(&ctx);
        self.song_editor_window(&ctx);
        self.medley_window(&ctx);
        self.update_window(&ctx);
        if self.about_open {
            egui::Window::new("About SimplePresenter")
                .open(&mut self.about_open)
                .collapsible(false)
                .show(&ctx, |ui| {
                    ui.heading(format!("SimplePresenter {}", env!("CARGO_PKG_VERSION")));
                    ui.label("Bible and lyrics projection, rewritten in pure Rust (egui).");
                    ui.hyperlink("https://github.com/psnact/SimplePresenter");
                });
        }
    }

    fn on_exit(&mut self) {
        self.stop_youtube();
        let _ = self.settings.save();
    }
}

#[cfg(test)]
mod media_import_tests {
    #[test]
    fn duplicate_names_get_a_counter() {
        let dir = std::env::temp_dir().join(format!("sp-media-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let src = std::path::Path::new("/elsewhere/hero.png");
        assert_eq!(super::unique_path(&dir, src), Some(dir.join("hero.png")));
        let _ = std::fs::write(dir.join("hero.png"), b"x");
        assert_eq!(super::unique_path(&dir, src), Some(dir.join("hero_1.png")));
        let _ = std::fs::write(dir.join("hero_1.png"), b"x");
        assert_eq!(super::unique_path(&dir, src), Some(dir.join("hero_2.png")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
