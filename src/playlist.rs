//! Service playlists stored as `.service` JSON.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
    #[serde(rename = "bible")]
    Bible,
    Song,
    Media,
    #[serde(rename = "youtube")]
    YouTube,
    #[serde(rename = "powerpoint")]
    PowerPoint,
}

impl ItemType {
    pub fn label(self) -> &'static str {
        match self {
            ItemType::Bible => "Bible",
            ItemType::Song => "Song",
            ItemType::Media => "Media",
            ItemType::YouTube => "YouTube",
            ItemType::PowerPoint => "PowerPoint",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    #[serde(rename = "type")]
    pub kind: ItemType,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub data: Map<String, Value>,
}

impl Item {
    /// What lists show: the title (or the reference when there is none), never a file
    /// extension or a full path — media shows the bare file name, like the original.
    pub fn display_title(&self) -> String {
        let raw = if self.title.trim().is_empty() {
            self.reference.as_str()
        } else {
            self.title.as_str()
        };
        if self.kind == ItemType::Media {
            let name = std::path::Path::new(raw)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .filter(|s| !s.is_empty());
            if let Some(name) = name {
                return name;
            }
        }
        raw.to_string()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Playlist {
    #[serde(default = "version")]
    pub version: String,
    #[serde(default)]
    pub items: Vec<Item>,
}

fn version() -> String {
    "1.0".into()
}

impl Playlist {
    pub fn load(path: &Path) -> Result<Playlist, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
        Self::parse(&text)
    }

    /// Unknown item types are skipped rather than failing the whole service.
    pub fn parse(text: &str) -> Result<Playlist, String> {
        let raw: Value =
            serde_json::from_str(text).map_err(|e| format!("Invalid service file: {e}"))?;
        let items = raw
            .get("items")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| serde_json::from_value::<Item>(v.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        Ok(Playlist {
            version: version(),
            items,
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let mut p = self.clone();
        p.version = version();
        let text = serde_json::to_string_pretty(&p).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("Cannot save {}: {e}", path.display()))
    }

    pub fn move_item(&mut self, from: usize, to: usize) {
        if from < self.items.len() && to < self.items.len() && from != to {
            let it = self.items.remove(from);
            self.items.insert(to, it);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let p = Playlist::parse(r#"{"version":"1.0","items":[{"type":"bible","title":"A","reference":"John 3:16","data":{}},{"type":"weird"},{"type":"youtube","title":"Y","reference":"u","data":{"url":"x"}}]}"#).unwrap();
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.items[1].kind, ItemType::YouTube);
        let s = serde_json::to_string(&p).unwrap();
        assert!(s.contains("\"type\":\"youtube\""));
        assert_eq!(Playlist::parse(&s).unwrap(), p);
    }

    #[test]
    fn display_title_hides_paths_and_extensions() {
        let media = Item {
            kind: ItemType::Media,
            title: "hero.png".into(),
            reference: "/lib/media/hero.png".into(),
            data: Default::default(),
        };
        assert_eq!(media.display_title(), "hero");
        let media_path = Item {
            kind: ItemType::Media,
            title: String::new(),
            reference: "/lib/media/clip.mp4".into(),
            data: Default::default(),
        };
        assert_eq!(media_path.display_title(), "clip");
        let song = Item {
            kind: ItemType::Song,
            title: "Amazing Grace".into(),
            reference: "Amazing Grace".into(),
            data: Default::default(),
        };
        assert_eq!(song.display_title(), "Amazing Grace");
    }
}
