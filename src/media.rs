//! Video and audio playback: DeckCraft's pure-Rust decoders, the `Player` mixer/clock and a cpal
//! output. Each slot (live media, background video) is one voice; video frames follow its clock.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use deckcraft_media::{
    Bytes, ClipParams, Frame, PlayState, Player, VideoFeed, VoiceSpec, VoiceStatus,
};
use egui::TextureHandle;

/// The media item on the live output.
pub const LIVE: u64 = 1;
/// Slot for the Media tab's preview pane (muted).
pub const PREVIEW: u64 = 3;
/// The looping background video behind text.
pub const BACKGROUND: u64 = 2;

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "bmp", "gif", "webp"];
pub const VIDEO_EXTS: &[&str] = &["mp4", "m4v", "mov", "webm", "mkv"];
pub const AUDIO_EXTS: &[&str] = &[
    "mp3", "m4a", "aac", "wav", "flac", "ogg", "opus", "aiff", "aif",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Video,
    Audio,
}

pub fn kind_of(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    let e = ext.as_str();
    if IMAGE_EXTS.contains(&e) {
        Some(Kind::Image)
    } else if VIDEO_EXTS.contains(&e) {
        Some(Kind::Video)
    } else if AUDIO_EXTS.contains(&e) {
        Some(Kind::Audio)
    } else {
        None
    }
}

struct Slot {
    path: String,
    volume: f32,
    feed: Option<VideoFeed>,
    frame: Option<Arc<Frame>>,
    tex: Option<TextureHandle>,
    still: Option<TextureHandle>,
}

pub struct Media {
    player: Player,
    slots: HashMap<u64, Slot>,
}

impl Default for Media {
    fn default() -> Self {
        Media {
            player: Player::new(Some(Box::new(crate::audio::CpalOut::default()))),
            slots: HashMap::new(),
        }
    }
}

fn media_key(path: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.hash(&mut h);
    h.finish()
}

impl Media {
    pub fn path(&self, slot: u64) -> Option<&str> {
        self.slots.get(&slot).map(|s| s.path.as_str())
    }

    /// Start `path` in `slot` from the beginning (replacing what was there).
    pub fn play(
        &mut self,
        slot: u64,
        path: &str,
        looping: bool,
        audio: bool,
        volume: f32,
    ) -> Result<(), String> {
        self.stop(slot);
        let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let bytes: Bytes = Arc::new(data);
        let duration = deckcraft_media::probe(&bytes)
            .map(|i| i.duration())
            .unwrap_or(0.0);
        let feed = if kind_of(Path::new(path)) == Some(Kind::Video) {
            match VideoFeed::new(bytes.clone()) {
                Ok(f) => Some(f),
                Err(e) => {
                    log::info!("video {path}: {e}");
                    if !audio {
                        return Err(format!("Can't play this video: {e}"));
                    }
                    None
                }
            }
        } else {
            None
        };
        let spec = VoiceSpec {
            clip: ClipParams {
                volume: volume.clamp(0.0, 1.0),
                ..ClipParams::default()
            },
            looping,
            duration,
            audio,
        };
        self.player.play(slot, media_key(path), &bytes, spec, None);
        self.slots.insert(
            slot,
            Slot {
                path: path.to_string(),
                volume: volume.clamp(0.0, 1.0),
                feed,
                frame: None,
                tex: None,
                still: None,
            },
        );
        Ok(())
    }

    /// Keep `slot` playing `path` (looping, silent): used for background videos.
    pub fn ensure(&mut self, slot: u64, path: &str) -> Result<(), String> {
        if self.path(slot) == Some(path) {
            return Ok(());
        }
        self.play(slot, path, true, false, 0.0)
    }

    pub fn toggle_pause(&mut self, slot: u64) {
        match self.player.status(slot).map(|s| s.state) {
            Some(PlayState::Playing | PlayState::Loading) => self.player.pause(slot),
            Some(PlayState::Paused) => {
                let pos = self.player.status(slot).map(|s| s.position);
                self.resume(slot, pos);
            }
            Some(PlayState::Ended) => self.resume(slot, Some(0.0)),
            None => {}
        }
    }

    fn resume(&mut self, slot: u64, from: Option<f64>) {
        let Some((path, volume)) = self.slots.get(&slot).map(|s| (s.path.clone(), s.volume)) else {
            return;
        };
        let Ok(data) = std::fs::read(&path) else {
            return;
        };
        let bytes: Bytes = Arc::new(data);
        let duration = deckcraft_media::probe(&bytes)
            .map(|i| i.duration())
            .unwrap_or(0.0);
        let audio = slot == LIVE;
        let spec = VoiceSpec {
            clip: ClipParams {
                volume,
                ..ClipParams::default()
            },
            looping: false,
            duration,
            audio,
        };
        self.player.play(slot, media_key(&path), &bytes, spec, from);
    }

    /// Change the volume of `slot` live (0.0 = silent, 1.0 = full).
    pub fn set_volume(&mut self, slot: u64, volume: f32) {
        let v = volume.clamp(0.0, 1.0);
        if let Some(s) = self.slots.get_mut(&slot) {
            s.volume = v;
        }
        self.player.set_volume(slot, v);
    }

    pub fn seek(&mut self, slot: u64, t: f64) {
        self.player.seek(slot, t);
    }

    pub fn stop(&mut self, slot: u64) {
        self.player.stop(slot);
        self.slots.remove(&slot);
    }

    pub fn status(&self, slot: u64) -> Option<VoiceStatus> {
        self.player.status(slot)
    }

    pub fn texture(&self, slot: u64) -> Option<TextureHandle> {
        self.slots.get(&slot).and_then(|s| s.tex.clone())
    }

    /// The first frame of the slot's video, for a paused preview.
    pub fn still(&self, slot: u64) -> Option<TextureHandle> {
        self.slots.get(&slot).and_then(|s| s.still.clone())
    }

    /// Per frame: advance the clock and upload due video frames. Returns whether anything plays.
    pub fn tick(&mut self, ctx: &egui::Context) -> bool {
        self.player.tick(ctx.input(|i| i.time));
        for (key, s) in &mut self.slots {
            let Some(st) = self.player.status(*key) else {
                continue;
            };
            let Some(feed) = s.feed.as_mut() else {
                continue;
            };
            if let Some(f) = feed.frame_at(st.position)
                && s.frame.as_ref().is_none_or(|old| !Arc::ptr_eq(old, &f))
            {
                let img = egui::ColorImage::from_rgba_unmultiplied(
                    [f.width as usize, f.height as usize],
                    &f.rgba,
                );
                if s.still.is_none() {
                    s.still = Some(ctx.load_texture(
                        format!("media-still-{key}"),
                        img.clone(),
                        egui::TextureOptions::LINEAR,
                    ));
                }
                match s.tex.as_mut() {
                    Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                    None => {
                        s.tex = Some(ctx.load_texture(
                            format!("media-{key}"),
                            img,
                            egui::TextureOptions::LINEAR,
                        ))
                    }
                }
                s.frame = Some(f);
            }
        }
        let playing = self.slots.keys().any(|k| {
            self.player
                .status(*k)
                .is_some_and(|s| matches!(s.state, PlayState::Playing | PlayState::Loading))
        });
        if playing {
            ctx.request_repaint_after(std::time::Duration::from_millis(15));
        }
        playing
    }
}

pub fn fmt_time(t: f64) -> String {
    let t = if t.is_finite() { t.max(0.0) as u64 } else { 0 };
    format!("{}:{:02}", t / 60, t % 60)
}
