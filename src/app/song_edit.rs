//! Song editor and medley builder windows.

use std::path::PathBuf;

use egui::{Color32, RichText, Ui};

use super::App;
use crate::paths;
use crate::songs::{self, Section, Song, SongLibrary};

#[derive(Default)]
pub(super) struct SongEditor {
    old_path: Option<PathBuf>,
    title: String,
    author: String,
    copyright: String,
    ccli: String,
    lyrics: String,
    marker: usize,
    error: String,
}

#[derive(Default)]
pub(super) struct MedleyBuilder {
    old_path: Option<PathBuf>,
    search: String,
    song: Option<String>,
    picked: Vec<usize>,
    draft: Vec<(String, Section)>,
    selected: Option<usize>,
    name: String,
    name_edited: bool,
    error: String,
}

impl SongEditor {
    pub(super) fn new_song() -> Self {
        Self::default()
    }

    pub(super) fn edit(song: &Song) -> Self {
        Self {
            old_path: Some(song.path.clone()).filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("xml") || e.eq_ignore_ascii_case("txt"))
            }),
            title: song.title.clone(),
            author: song.author.clone(),
            copyright: song.copyright.clone(),
            ccli: song.ccli.clone(),
            lyrics: songs::editor_text(song),
            ..Default::default()
        }
    }

    /// Label the paragraph at the end of the lyrics' last non-empty block (egui has no
    /// portable cursor access here, so the marker goes before the paragraph holding `at`).
    fn insert_marker(&mut self, at: usize) {
        let label = songs::SECTION_TYPES
            .get(self.marker)
            .copied()
            .unwrap_or("Verse");
        let at = self
            .lyrics
            .char_indices()
            .map(|(i, _)| i)
            .chain([self.lyrics.len()])
            .nth(at)
            .unwrap_or(self.lyrics.len());
        let start = self.lyrics[..at].rfind("\n\n").map(|i| i + 2).unwrap_or(0);
        let label = if label == "Verse" {
            let n = self.lyrics[..start]
                .lines()
                .filter(|l| l.trim().to_lowercase().starts_with("[verse"))
                .count()
                + 1;
            format!("Verse {n}")
        } else {
            label.to_string()
        };
        let line_end = self.lyrics[start..]
            .find('\n')
            .map(|i| start + i)
            .unwrap_or(self.lyrics.len());
        let first = self.lyrics[start..line_end].trim();
        if first.starts_with('[') && first.ends_with(']') {
            self.lyrics
                .replace_range(start..line_end, &format!("[{label}]"));
        } else {
            self.lyrics.insert_str(start, &format!("[{label}]\n"));
        }
    }
}

impl MedleyBuilder {
    pub(super) fn edit(song: &Song) -> Self {
        let draft = song
            .sections
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let src = s
                    .label
                    .split_once(" \u{2014} ")
                    .map(|(_, t)| t.to_string())
                    .unwrap_or_default();
                let mut s = s.clone();
                if s.label.is_empty() {
                    s.label = song.display_label(i);
                }
                (src, s)
            })
            .collect();
        Self {
            old_path: Some(song.path.clone()),
            draft,
            name: song.title.clone(),
            name_edited: true,
            ..Default::default()
        }
    }

    fn default_name(&mut self) {
        if self.name_edited {
            return;
        }
        let mut names: Vec<&str> = Vec::new();
        for (t, _) in &self.draft {
            if !t.is_empty() && !names.contains(&t.as_str()) {
                names.push(t);
            }
        }
        self.name = names.join(" + ");
    }

    fn song(&self) -> Song {
        Song {
            title: self.name.trim().to_string(),
            medley: true,
            sections: self.draft.iter().map(|(_, s)| s.clone()).collect(),
            ..Default::default()
        }
    }
}

fn section_color(s: &Section) -> Color32 {
    let [r, g, b] = songs::section_color(&s.kind);
    Color32::from_rgb(r, g, b)
}

impl App {
    pub(super) fn song_editor_window(&mut self, ctx: &egui::Context) {
        let Some(ed) = self.song_editor.as_mut() else {
            return;
        };
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        let title = if ed.old_path.is_some() {
            "Edit Song"
        } else {
            "Add New Song"
        };
        egui::Window::new(title).open(&mut open).default_size([560.0, 640.0]).show(ctx, |ui| {
            egui::Grid::new("song_info").num_columns(2).show(ui, |ui| {
                for (label, v) in [("Title", &mut ed.title), ("Author", &mut ed.author), ("Copyright", &mut ed.copyright), ("CCLI", &mut ed.ccli)] {
                    ui.label(label);
                    ui.add(egui::TextEdit::singleline(v).desired_width(380.0));
                    ui.end_row();
                }
            });
            ui.separator();
            ui.label(
                RichText::new("Separate sections with a blank line. Label a section with a line like [Chorus] above it; a marker applies until the next marker.")
                    .small(),
            );
            let mut insert = false;
            ui.horizontal(|ui| {
                ui.label("Section type");
                egui::ComboBox::from_id_salt("marker").selected_text(songs::SECTION_TYPES.get(ed.marker).copied().unwrap_or("Verse")).show_ui(ui, |ui| {
                    for (i, t) in songs::SECTION_TYPES.iter().enumerate() {
                        ui.selectable_value(&mut ed.marker, i, *t);
                    }
                });
                insert = ui.button("Insert Marker at Cursor").on_hover_text("Labels the section containing the cursor, or renames its marker").clicked();
            });
            let id = egui::Id::new("song_lyrics");
            let out = egui::ScrollArea::vertical()
                .max_height((ui.available_height() - 40.0).max(160.0))
                .show(ui, |ui| {
                    egui::TextEdit::multiline(&mut ed.lyrics)
                        .id(id)
                        .desired_width(f32::INFINITY)
                        .desired_rows(20)
                        .hint_text("First verse line 1\nFirst verse line 2\n\nChorus line 1\nChorus line 2")
                        .show(ui)
                })
                .inner;
            if insert {
                let at = out.cursor_range.map(|c| c.primary.index.0).unwrap_or(ed.lyrics.chars().count());
                ed.insert_marker(at);
            }
            if !ed.error.is_empty() {
                ui.colored_label(Color32::LIGHT_RED, &ed.error);
            }
            ui.horizontal(|ui| {
                save = ui.button("Save").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if save {
            let sections = songs::parse_editor_text(&ed.lyrics);
            ed.error = if ed.title.trim().is_empty() {
                "Please enter a song title.".into()
            } else if sections.is_empty() {
                "Please enter song lyrics.".into()
            } else {
                String::new()
            };
            if ed.error.is_empty() {
                let song = Song {
                    title: ed.title.trim().into(),
                    author: ed.author.trim().into(),
                    copyright: ed.copyright.trim().into(),
                    ccli: ed.ccli.trim().into(),
                    sections,
                    ..Default::default()
                };
                match songs::save(&song, &paths::songs_dir(), ed.old_path.as_deref()) {
                    Ok(_) => {
                        self.reload_songs(Some(song.title.clone()));
                        self.status = format!("Saved song \"{}\"", song.title);
                        self.song_editor = None;
                        return;
                    }
                    Err(e) => ed.error = e,
                }
            }
        }
        if !open || cancel {
            self.song_editor = None;
        }
    }

    pub(super) fn medley_window(&mut self, ctx: &egui::Context) {
        let Some(mb) = self.medley.as_mut() else {
            return;
        };
        let library: &SongLibrary = &self.songs;
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        let title = if mb.old_path.is_some() {
            "Edit Medley"
        } else {
            "Medley Builder"
        };
        egui::Window::new(title)
            .open(&mut open)
            .default_size([900.0, 560.0])
            .min_height(480.0)
            .show(ctx, |ui| {
                let h = 400.0;
                ui.columns(3, |cols| {
                    let ui: &mut Ui = &mut cols[0];
                    ui.set_min_height(h + 40.0);
                    ui.label("Songs");
                    ui.text_edit_singleline(&mut mb.search);
                    egui::ScrollArea::vertical()
                        .id_salt("m_songs")
                        .max_height(h)
                        .show(ui, |ui| {
                            for t in library.search(&mb.search) {
                                if ui
                                    .selectable_label(mb.song.as_deref() == Some(&t), &t)
                                    .clicked()
                                {
                                    mb.song = Some(t);
                                    mb.picked.clear();
                                }
                            }
                        });
                    let ui: &mut Ui = &mut cols[1];
                    ui.label("Sections (click to select, double-click to add)");
                    let song = mb.song.as_ref().and_then(|t| library.get(t));
                    let mut add_now: Vec<usize> = Vec::new();
                    egui::ScrollArea::vertical()
                        .id_salt("m_secs")
                        .max_height(h)
                        .show(ui, |ui| {
                            if let Some(song) = song {
                                for (i, s) in song.sections.iter().enumerate() {
                                    let on = mb.picked.contains(&i);
                                    let r = egui::Frame::group(ui.style())
                                        .fill(if on {
                                            section_color(s).gamma_multiply(0.45)
                                        } else {
                                            ui.visuals().faint_bg_color
                                        })
                                        .stroke(egui::Stroke::new(1.0, section_color(s)))
                                        .show(ui, |ui| {
                                            ui.set_width(ui.available_width());
                                            ui.label(
                                                RichText::new(song.display_label(i))
                                                    .strong()
                                                    .color(section_color(s)),
                                            );
                                            ui.label(s.text());
                                        })
                                        .response
                                        .interact(egui::Sense::click());
                                    if r.double_clicked() {
                                        add_now.push(i);
                                    } else if r.clicked() {
                                        if on {
                                            mb.picked.retain(|x| *x != i);
                                        } else {
                                            mb.picked.push(i);
                                        }
                                    }
                                }
                            }
                        });
                    if ui
                        .add_enabled(!mb.picked.is_empty(), egui::Button::new("Add ▶"))
                        .clicked()
                    {
                        add_now = std::mem::take(&mut mb.picked);
                        add_now.sort_unstable();
                    }
                    if let Some(song) = song
                        && !add_now.is_empty()
                    {
                        for i in add_now {
                            if let Some(s) = song.sections.get(i) {
                                let mut s = s.clone();
                                s.label =
                                    format!("{} \u{2014} {}", song.display_label(i), song.title);
                                mb.draft.push((song.title.clone(), s));
                            }
                        }
                        mb.selected = Some(mb.draft.len().saturating_sub(1));
                        mb.default_name();
                    }
                    let ui: &mut Ui = &mut cols[2];
                    ui.label("Medley order");
                    egui::ScrollArea::vertical()
                        .id_salt("m_draft")
                        .max_height(h - 30.0)
                        .show(ui, |ui| {
                            for (i, (_, s)) in mb.draft.iter().enumerate() {
                                let sel = mb.selected == Some(i);
                                let r = egui::Frame::group(ui.style())
                                    .fill(section_color(s).gamma_multiply(if sel {
                                        0.6
                                    } else {
                                        0.25
                                    }))
                                    .stroke(egui::Stroke::new(
                                        if sel { 2.0 } else { 1.0 },
                                        section_color(s),
                                    ))
                                    .show(ui, |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.label(RichText::new(&s.label).strong());
                                        ui.label(s.text());
                                    })
                                    .response
                                    .interact(egui::Sense::click());
                                if r.clicked() {
                                    mb.selected = Some(i);
                                }
                            }
                        });
                    ui.horizontal(|ui| {
                        let sel = mb.selected.filter(|i| *i < mb.draft.len());
                        if ui
                            .add_enabled(sel.is_some_and(|i| i > 0), egui::Button::new("Move Up"))
                            .clicked()
                            && let Some(i) = sel
                        {
                            mb.draft.swap(i - 1, i);
                            mb.selected = Some(i - 1);
                        }
                        if ui
                            .add_enabled(
                                sel.is_some_and(|i| i + 1 < mb.draft.len()),
                                egui::Button::new("Move Down"),
                            )
                            .clicked()
                            && let Some(i) = sel
                        {
                            mb.draft.swap(i, i + 1);
                            mb.selected = Some(i + 1);
                        }
                        if ui
                            .add_enabled(sel.is_some(), egui::Button::new("Remove"))
                            .clicked()
                            && let Some(i) = sel
                        {
                            mb.draft.remove(i);
                            mb.selected = None;
                            mb.default_name();
                        }
                    });
                });
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("Medley name");
                    if ui
                        .add(egui::TextEdit::singleline(&mut mb.name).desired_width(320.0))
                        .changed()
                    {
                        mb.name_edited = true;
                    }
                    save = ui
                        .add_enabled(!mb.draft.is_empty(), egui::Button::new("Save Medley"))
                        .clicked();
                    cancel = ui.button("Cancel").clicked();
                });
                if !mb.error.is_empty() {
                    ui.colored_label(Color32::LIGHT_RED, &mb.error);
                }
            });
        if save {
            if mb.name.trim().is_empty() {
                mb.error = "Please enter a medley name.".into();
            } else {
                let song = mb.song();
                match songs::save(&song, &paths::songs_dir(), mb.old_path.as_deref()) {
                    Ok(_) => {
                        self.reload_songs(Some(song.title.clone()));
                        self.status = format!("Saved medley \"{}\"", song.title);
                        self.medley = None;
                        return;
                    }
                    Err(e) => mb.error = e,
                }
            }
        }
        if !open || cancel {
            self.medley = None;
        }
    }

    pub(super) fn reload_songs(&mut self, select: Option<String>) {
        self.songs = SongLibrary::load_dir(&paths::songs_dir());
        if select.is_some() {
            self.song_ui.selected = select;
        }
    }

    pub(super) fn delete_song(&mut self, title: &str) {
        let Some(path) = self.songs.get(title).map(|s| s.path.clone()) else {
            return;
        };
        match std::fs::remove_file(&path) {
            Ok(()) => {
                self.song_ui.selected = None;
                self.reload_songs(None);
                self.status = format!("Deleted \"{title}\"");
            }
            Err(e) => self.status = format!("Could not delete {}: {e}", path.display()),
        }
    }
}
