//! Feeds the overlay server (OBS overlay, projection stream) and handles phone-remote commands.

use std::sync::Arc;

use serde_json::{Value, json};

use super::*;
use crate::server::{self, Server};

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// This computer's LAN address, for showing the remote URL (no packets are sent).
pub fn lan_ip() -> String {
    std::net::UdpSocket::bind("0.0.0.0:0")
        .and_then(|s| {
            s.connect("8.8.8.8:80")?;
            s.local_addr()
        })
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|_| "localhost".into())
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

impl App {
    pub(super) fn start_server(&mut self) {
        self.server = None;
        if !self.settings.overlay_server_enabled {
            self.server_error.clear();
            return;
        }
        match Server::start(
            self.settings.overlay_server_port,
            paths::resource_dir(),
            self.egui_ctx.clone(),
        ) {
            Ok(s) => {
                self.server_error.clear();
                self.server = Some(s);
            }
            Err(e) => {
                self.server_error = format!("Overlay server could not start: {e}");
                self.status = self.server_error.clone();
            }
        }
    }

    pub(super) fn server_url(&self, page: &str) -> Option<String> {
        self.server
            .as_ref()
            .map(|s| format!("http://{}:{}/{page}", lan_ip(), s.port))
    }

    fn now_showing(&self) -> String {
        match &self.content {
            Content::Empty => String::new(),
            Content::Bible { reference, .. } => reference.clone(),
            Content::Lyrics { title, .. } => title.clone(),
            Content::Picture { key } => file_name(key),
            Content::Video { title, .. } | Content::YouTube { title, .. } => title.clone(),
        }
    }

    pub(super) fn server_frame(&mut self, ctx: &egui::Context) {
        let Some(server) = self.server.take() else {
            return;
        };
        for cmd in server.commands() {
            self.remote_command(&server, &cmd);
            ctx.request_repaint();
        }
        self.publish(&server);
        self.server = Some(server);
    }

    fn publish(&mut self, server: &Server) {
        let (mut d, mut media_path) = (server::Data::default(), String::new());
        match &self.content {
            Content::Bible { text, reference } => {
                d.reference = crate::canvas::reference_text(
                    reference,
                    self.settings.obs_overlay.display_version,
                )
                .to_string();
                d.text = server::bracket_plain(text);
                d.text_html = server::bracket_html(text);
            }
            Content::Lyrics { text, .. } => {
                d.text = server::bracket_plain(text);
                d.text_html = server::bracket_html(text);
            }
            Content::Picture { key } => media_path = key.clone(),
            Content::Video { path, .. } => {
                if media::kind_of(std::path::Path::new(path)) == Some(media::Kind::Video) {
                    media_path = path.clone();
                    d.media_is_video = true;
                }
            }
            Content::YouTube { url, .. } => {
                d.has_youtube = true;
                d.youtube_url = crate::browser::embed_url(url, true);
            }
            Content::Empty => {}
        }
        if media_path != self.server_media_path {
            self.server_media_path = media_path.clone();
            self.server_media_ts = now_ms();
        }
        let st = self
            .media
            .status(media::LIVE)
            .filter(|_| matches!(self.content, Content::Video { .. }));
        d.has_media = !media_path.is_empty();
        d.media_timestamp = self.server_media_ts;
        if let Some(st) = &st {
            d.media_playing = matches!(
                st.state,
                deckcraft_media::PlayState::Playing | deckcraft_media::PlayState::Loading
            );
            d.media_position_ms = (st.position.max(0.0) * 1000.0) as i64;
        }
        d.blackout = self.black;
        d.media_fullscreen = self.media_fullscreen;
        let mut cfg = self.overlay();
        let obs = &self.settings.obs_overlay;
        cfg.text = obs.text.clone();
        cfg.reference = obs.reference.clone();
        cfg.reference_pos = obs.reference_pos;
        cfg.reference_spacing = obs.reference_spacing;
        cfg.display_version = obs.display_version;
        if self.server_style.as_ref() != Some(&cfg) {
            self.server_style = Some(cfg.clone());
            self.server_style_version += 1;
        }
        d.style_version = self.server_style_version;
        server.update(|s| {
            s.data = d;
            s.media_path = media_path;
            if s.config != cfg {
                s.config = cfg;
            }
        });

        let mut state = json!({
            "type": "remoteState",
            "nowShowing": self.now_showing(),
            "mediaPlaying": st.is_some(),
            "mediaPaused": st.as_ref().is_some_and(|s| !matches!(s.state, deckcraft_media::PlayState::Playing | deckcraft_media::PlayState::Loading)),
            // Whole seconds, so the state only re-broadcasts about once a second while playing.
            "mediaPositionMs": st.as_ref().map(|s| (s.position.max(0.0).floor() * 1000.0) as i64).unwrap_or(0),
            "mediaDurationMs": st.as_ref().map(|s| (s.duration.max(0.0) * 1000.0) as i64).unwrap_or(-1),
            "mediaVolume": (self.media_volume * 100.0).round() as i64,
            "mediaFullscreen": self.media_fullscreen,
        });
        if let Source::Song { title, section } = &self.source
            && let Some(song) = self.songs.get(title)
            && let Some(obj) = state.as_object_mut()
        {
            let sections: Vec<Value> = song
                .sections
                .iter()
                .enumerate()
                .map(|(i, sec)| {
                    let [r, g, b] = crate::songs::section_color(&sec.kind);
                    let preview = sec.lines.iter().map(|l| l.trim()).find(|l| !l.is_empty()).unwrap_or_default();
                    json!({ "label": song.display_label(i), "color": format!("#{r:02x}{g:02x}{b:02x}"), "preview": preview })
                })
                .collect();
            obj.insert("song".into(), json!({ "title": song.title, "sections": sections, "current": section, "labeled": true }));
        }
        server.set_remote_state(state);

        let slides_key = self
            .deck
            .as_ref()
            .map(|d| (d.path.clone(), d.rendered() + usize::from(!d.rendering())))
            .unwrap_or_default();
        if slides_key != self.server_slides {
            self.server_slides = slides_key;
            let jpegs = self
                .deck
                .as_ref()
                .map(|d| d.jpegs.clone())
                .unwrap_or_default();
            server.update(|s| s.slides = jpegs);
            if self.deck.as_ref().is_none_or(|d| !d.rendering()) {
                server.send(&self.ppt_state_msg());
            }
        }
    }

    /// Render the live picture for `/projection.jpg` and `/projection.mjpg` while someone watches.
    pub(super) fn capture(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        let Some(server) = self.server.take() else {
            return;
        };
        let rs = frame.wgpu_render_state();
        if let Some(jpeg) = self.offscreen.take(rs).as_ref().and_then(encode_jpeg) {
            server.update(|s| {
                s.frame_jpeg = Arc::new(jpeg);
                s.frame_seq = s.frame_seq.wrapping_add(1);
            });
        }
        let now = ctx.input(|i| i.time);
        if let Some(rs) = rs
            && server.frames_wanted()
            && now - self.frame_requested >= 0.2
        {
            self.frame_requested = now;
            let ppp = ctx.pixels_per_point();
            let [ow, oh] = self.settings.output_size;
            let scale = (1920.0 / ow.max(1) as f32).min(1.0);
            let size = [
                ((ow as f32 * scale).round() as u32).max(16),
                ((oh as f32 * scale).round() as u32).max(16),
            ];
            let layer = egui::LayerId::new(egui::Order::Background, egui::Id::new("sp_offscreen"));
            ctx.set_transform_layer(
                layer,
                egui::emath::TSTransform::from_translation(egui::vec2(-1.0e6, -1.0e6)),
            );
            let rect = egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(size[0] as f32, size[1] as f32) / ppp,
            );
            let painter = egui::Painter::new(ctx.clone(), layer, rect);
            self.paint_live_on(ctx, &painter, rect, false);
            let shapes = ctx.graphics_mut(|g| g.entry(layer).all_entries().cloned().collect());
            self.offscreen.render(rs, ctx, shapes, size, ppp);
        }
        if server.frames_wanted() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.server = Some(server);
    }

    fn bible_results(&self, query: &str) -> Value {
        let Some(b) = &self.bible else {
            return json!({ "type": "bibleResults", "query": query, "items": [] });
        };
        let items: Vec<Value> = match b.parse_reference(query) {
            Some(r) => {
                let (s, e) = if r.chapter_only {
                    (0, 0)
                } else {
                    (r.start, r.end)
                };
                b.verses(&r.book, r.chapter, 1, u32::MAX)
                    .iter()
                    .map(|v| json!({ "ref": v.reference(), "text": v.text, "hit": v.verse >= s && v.verse <= e }))
                    .collect()
            }
            None => b
                .search(query, 100)
                .iter()
                .map(|v| json!({ "ref": v.reference(), "text": v.text, "hit": false }))
                .collect(),
        };
        json!({ "type": "bibleResults", "query": query, "translation": b.acronym, "items": items })
    }

    fn current_bible_ref(&self) -> Option<String> {
        match &self.source {
            Source::Bible {
                book,
                chapter,
                start,
                ..
            } => Some(format!("{book} {chapter}:{start}")),
            _ => None,
        }
    }

    fn playlist_msg(&self) -> Value {
        let items: Vec<Value> = self.playlist.items.iter().enumerate().map(|(i, it)| json!({ "index": i, "title": it.display_title(), "kind": it.kind.label().to_lowercase() })).collect();
        json!({ "type": "playlist", "items": items, "current": self.playlist_sel.map(|i| i as i64).unwrap_or(-1) })
    }

    fn media_list(&self) -> Vec<Value> {
        let root = paths::media_dir();
        let mut out = Vec::new();
        let mut dirs = vec![(root.clone(), ".".to_string())];
        while let Some((dir, folder)) = dirs.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            let mut entries: Vec<_> = rd.flatten().map(|e| e.path()).collect();
            entries.sort();
            for p in entries {
                if p.is_dir() {
                    if dirs.len() < 64 {
                        let name = p
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        dirs.push((
                            p.clone(),
                            if folder == "." {
                                name
                            } else {
                                format!("{folder}/{name}")
                            },
                        ));
                    }
                } else if let Some(k) = media::kind_of(&p) {
                    let path = p.to_string_lossy().into_owned();
                    out.push(json!({ "name": file_name(&path), "path": path, "isVideo": k != media::Kind::Image, "folder": folder }));
                }
            }
        }
        out
    }

    fn ppt_library_msg(&self) -> Value {
        let items: Vec<Value> = crate::ppt::library()
            .iter()
            .map(|p| json!({ "name": crate::ppt::name_of(p), "path": p.to_string_lossy(), "active": self.deck.as_ref().is_some_and(|d| &d.path == p) }))
            .collect();
        json!({ "type": "pptlibrary", "items": items })
    }

    fn ppt_state_msg(&self) -> Value {
        let Some(d) = &self.deck else {
            return json!({ "type": "pptstate", "deck": "", "rendering": false, "slides": [] });
        };
        let current = match &self.source {
            Source::Slide { path, index } if *path == d.path => (*index + 1) as i64,
            _ => 0,
        };
        let slides: Vec<Value> = (0..d.count)
            .map(|i| json!({ "index": i + 1, "label": format!("Slide {}", i + 1) }))
            .collect();
        json!({ "type": "pptstate", "deck": d.name, "path": d.path.to_string_lossy(), "rendering": d.rendering(), "current": current, "slides": slides })
    }

    fn remote_command(&mut self, server: &Server, cmd: &Value) {
        let action = cmd
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let arg = cmd
            .get("arg")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let index = cmd.get("index").and_then(Value::as_i64).unwrap_or(-1);
        let idx = usize::try_from(index).ok();
        match action {
            "next" | "prev" => self.step(action == "next"),
            "biblenext" | "bibleprev" => {
                self.step(action == "biblenext");
                if let Some(r) = self.current_bible_ref() {
                    server.send(&self.bible_results(&r));
                }
            }
            "clear" => self.clear(),
            "blackout" => self.black = !self.black,
            "section" => {
                if let (Source::Song { title, .. }, Some(i)) = (self.source.clone(), idx) {
                    self.project_song(&title, i);
                } else if let (Some(t), Some(i)) = (self.song_ui.selected.clone(), idx) {
                    self.project_song(&t, i);
                }
            }
            "songs" => {
                let items: Vec<Value> = self.songs.titles().iter().map(|t| json!({ "title": t, "medley": self.songs.get(t).is_some_and(|s| s.medley) })).collect();
                server.send(&json!({ "type": "songs", "items": items }));
            }
            "song" => {
                self.project_song(&arg, 0);
            }
            "bible" | "biblego" => {
                let is_ref = self
                    .bible
                    .as_ref()
                    .and_then(|b| b.parse_reference(&arg))
                    .is_some();
                if action == "biblego" && is_ref {
                    self.project_reference(&arg);
                }
                server.send(&self.bible_results(&arg));
            }
            "verse" => {
                if self.project_reference(&arg) {
                    server.send(&self.bible_results(&arg));
                }
            }
            "translations" => {
                let items: Vec<Value> = self
                    .bibles
                    .iter()
                    .map(|p| {
                        let name = p.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        json!({ "name": name, "current": p.to_string_lossy() == self.settings.last_bible })
                    })
                    .collect();
                server.send(&json!({ "type": "translations", "items": items }));
            }
            "books" => {
                let items = self
                    .bible
                    .as_ref()
                    .map(|b| b.book_names())
                    .unwrap_or_default();
                server.send(&json!({ "type": "books", "items": items }));
            }
            "settranslation" => {
                if let Some(i) = self
                    .bibles
                    .iter()
                    .position(|p| p.file_stem().is_some_and(|n| n.to_string_lossy() == arg))
                {
                    self.load_bible(i);
                }
            }
            "media" => server.send(&json!({ "type": "media", "items": self.media_list() })),
            "projectmedia" => {
                let label = file_name(&arg);
                self.project_media(&arg, label);
            }
            "playlist" => server.send(&self.playlist_msg()),
            "plitem" => {
                if let Some(i) = idx {
                    self.activate_item(i);
                }
                server.send(&self.playlist_msg());
            }
            "plmove" => {
                let dir = cmd.get("dir").and_then(Value::as_i64).unwrap_or(0);
                let n = self.playlist.items.len() as i64;
                let target = index.saturating_add(dir);
                if index >= 0 && index < n && target >= 0 && target < n && dir != 0 {
                    self.playlist.move_item(index as usize, target as usize);
                    self.playlist_dirty = true;
                    if self.playlist_sel == Some(index as usize) {
                        self.playlist_sel = Some(target as usize);
                    } else if self.playlist_sel == Some(target as usize) {
                        self.playlist_sel = Some(index as usize);
                    }
                }
                server.send(&self.playlist_msg());
            }
            "plremove" => {
                if let Some(i) = idx.filter(|i| *i < self.playlist.items.len()) {
                    self.playlist.items.remove(i);
                    self.playlist_dirty = true;
                    self.playlist_sel = match self.playlist_sel {
                        Some(s) if s == i => None,
                        Some(s) if s > i => Some(s - 1),
                        s => s,
                    };
                }
                server.send(&self.playlist_msg());
            }
            "pladd" => {
                let kind = cmd.get("kind").and_then(Value::as_str).unwrap_or_default();
                let name = cmd.get("name").and_then(Value::as_str).unwrap_or_default();
                let item = match kind {
                    "bible" => Some(Item {
                        kind: ItemType::Bible,
                        title: arg.clone(),
                        reference: arg.clone(),
                        data: Default::default(),
                    }),
                    "song" => Some(Item {
                        kind: ItemType::Song,
                        title: arg.clone(),
                        reference: arg.clone(),
                        data: Default::default(),
                    }),
                    "songsection" => match &self.source {
                        Source::Song { title, .. } => Some(Item {
                            kind: ItemType::Song,
                            title: title.clone(),
                            reference: title.clone(),
                            data: Default::default(),
                        }),
                        _ => None,
                    },
                    "powerpoint" => {
                        let mut data = serde_json::Map::new();
                        data.insert("path".into(), Value::String(arg.clone()));
                        Some(Item {
                            kind: ItemType::PowerPoint,
                            title: crate::ppt::name_of(std::path::Path::new(&arg)),
                            reference: arg.clone(),
                            data,
                        })
                    }
                    "media" => {
                        let mut data = serde_json::Map::new();
                        data.insert("path".into(), Value::String(arg.clone()));
                        let title = if name.is_empty() {
                            file_name(&arg)
                        } else {
                            name.to_string()
                        };
                        Some(Item {
                            kind: ItemType::Media,
                            title,
                            reference: arg.clone(),
                            data,
                        })
                    }
                    _ => None,
                };
                let mut added = false;
                if let Some(item) = item {
                    match self
                        .playlist
                        .items
                        .iter()
                        .position(|i| i.kind == item.kind && i.reference == item.reference)
                    {
                        Some(i) => self.playlist_sel = Some(i),
                        None => {
                            self.add_to_playlist(item);
                            added = true;
                        }
                    }
                }
                let label = cmd.get("label").and_then(Value::as_str).unwrap_or_default();
                server.send(&json!({ "type": "pladdack", "added": added, "name": label }));
                server.send(&self.playlist_msg());
            }
            "mediaplaypause" => self.media.toggle_pause(media::LIVE),
            "mediaseek" => {
                if let Some(ms) = cmd
                    .get("positionMs")
                    .and_then(Value::as_f64)
                    .filter(|m| m.is_finite() && *m >= 0.0)
                {
                    self.media.seek(media::LIVE, ms / 1000.0);
                }
            }
            "mediavolume" => {
                if let Some(v) = cmd
                    .get("value")
                    .and_then(Value::as_f64)
                    .filter(|v| v.is_finite())
                {
                    self.media_volume = (v / 100.0).clamp(0.0, 1.0) as f32;
                    self.media.set_volume(media::LIVE, self.media_volume);
                }
            }
            "mediafullscreen" => self.media_fullscreen = !self.media_fullscreen,
            "pptlibrary" => server.send(&self.ppt_library_msg()),
            "pptstate" => server.send(&self.ppt_state_msg()),
            "pptload" => {
                let p = std::path::PathBuf::from(&arg);
                if crate::ppt::is_deck(&p) && p.exists() {
                    self.tab = Tab::PowerPoint;
                    self.open_deck(&p);
                }
                server.send(&self.ppt_state_msg());
            }
            "pptslide" => {
                if let (Some(d), Some(i)) = (&self.deck, idx.and_then(|i| i.checked_sub(1))) {
                    let p = d.path.clone();
                    self.project_slide(&p, i);
                }
            }
            "pptnext" | "pptprev" => {
                if matches!(self.source, Source::Slide { .. }) {
                    self.step(action == "pptnext");
                } else if let Some(d) = &self.deck {
                    let p = d.path.clone();
                    self.project_slide(&p, 0);
                }
            }
            _ => {}
        }
    }
}

fn encode_jpeg(img: &egui::ColorImage) -> Option<Vec<u8>> {
    let [w, h] = img.size;
    let mut rgb = Vec::with_capacity(w.checked_mul(h)?.checked_mul(3)?);
    for p in &img.pixels {
        rgb.extend_from_slice(&[p.r(), p.g(), p.b()]);
    }
    let buf = image::RgbImage::from_raw(u32::try_from(w).ok()?, u32::try_from(h).ok()?, rgb)?;
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80)
        .encode_image(&buf)
        .ok()?;
    Some(out.into_inner())
}
