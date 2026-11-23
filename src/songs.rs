//! Song library: XML and plain-text songs split into sections.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::Event;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Section {
    pub kind: String,
    pub label: String,
    pub lines: Vec<String>,
}

impl Section {
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Song {
    pub title: String,
    pub author: String,
    pub copyright: String,
    pub ccli: String,
    pub path: PathBuf,
    pub medley: bool,
    pub sections: Vec<Section>,
}

pub const SECTION_TYPES: &[&str] = &[
    "Verse",
    "Chorus",
    "Pre-Chorus",
    "Bridge",
    "Tag",
    "Intro",
    "Interlude",
    "Instrumental",
    "Outro",
    "Ending",
    "Other",
];

impl Song {
    pub fn display_label(&self, index: usize) -> String {
        let Some(s) = self.sections.get(index) else {
            return String::new();
        };
        if !s.label.is_empty() {
            return s.label.clone();
        }
        if s.kind != "verse" {
            return display_name_for_type(&s.kind);
        }
        let n = self
            .sections
            .iter()
            .take(index + 1)
            .filter(|x| x.kind == "verse")
            .count();
        format!("Verse {n}")
    }

    /// True when sections carry explicit labels or non-verse types; the original then
    /// shows a bold header row above each run of same-labelled sections.
    pub fn has_labeled_sections(&self) -> bool {
        self.sections
            .iter()
            .any(|s| !s.label.is_empty() || s.kind != "verse")
    }

    pub fn full_text(&self) -> String {
        self.sections
            .iter()
            .map(Section::text)
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

pub fn display_name_for_type(kind: &str) -> String {
    kind.split('-')
        .map(|p| {
            let mut c = p.chars();
            c.next()
                .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// "Chorus — Oceans" -> "chorus", "Verse 2" -> "verse".
pub fn section_type_for_label(label: &str) -> String {
    let mut base = label.trim();
    if let Some(i) = base.find(" \u{2014} ") {
        base = base[..i].trim();
    }
    base.trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace())
        .to_lowercase()
}

pub fn section_color(kind: &str) -> [u8; 3] {
    match kind.to_lowercase().as_str() {
        "verse" => [0x2E, 0x86, 0xC1],
        "chorus" => [0x23, 0x9B, 0x56],
        "pre-chorus" => [0x17, 0xA5, 0x89],
        "bridge" => [0x88, 0x4E, 0xA0],
        "tag" => [0xD6, 0x89, 0x10],
        "intro" | "outro" => [0x7F, 0x8C, 0x8D],
        "interlude" => [0x54, 0x99, 0xC7],
        "instrumental" => [0x56, 0x65, 0x73],
        "ending" => [0x93, 0x51, 0x16],
        _ => [0x5D, 0x6D, 0x7E],
    }
}

fn text_of(reader: &mut Reader<&[u8]>, end: &[u8]) -> String {
    let mut out = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(t)) => {
                let s = t.decode().map(|c| c.into_owned()).unwrap_or_default();
                out.push_str(
                    &quick_xml::escape::unescape(&s)
                        .map(|c| c.into_owned())
                        .unwrap_or(s),
                );
            }
            Ok(Event::GeneralRef(r)) => {
                let name = r.decode().map(|c| c.into_owned()).unwrap_or_default();
                out.push_str(
                    &quick_xml::escape::unescape(&format!("&{name};"))
                        .map(|c| c.into_owned())
                        .unwrap_or_default(),
                );
            }
            Ok(Event::CData(t)) => out.push_str(&String::from_utf8_lossy(&t)),
            Ok(Event::End(e)) if e.name().as_ref() == end => break,
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

pub fn parse_xml(xml: &str, path: &Path) -> Result<Song, String> {
    let mut reader = Reader::from_reader(xml.as_bytes());
    let mut song = Song {
        path: path.to_path_buf(),
        ..Default::default()
    };
    let mut cur: Option<Section> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let attr = |n: &[u8]| {
                    e.attributes()
                        .flatten()
                        .find(|a| a.key.as_ref() == n)
                        .and_then(|a| a.unescape_value().ok().map(|v| v.trim().to_string()))
                        .unwrap_or_default()
                };
                match e.name().as_ref() {
                    b"song" => {
                        song.medley = matches!(attr(b"medley").as_str(), "true" | "1" | "yes")
                    }
                    b"title" => song.title = text_of(&mut reader, b"title"),
                    b"author" => song.author = text_of(&mut reader, b"author"),
                    b"copyright" => song.copyright = text_of(&mut reader, b"copyright"),
                    b"ccli" => song.ccli = text_of(&mut reader, b"ccli"),
                    b"section" => {
                        let mut kind = attr(b"type").to_lowercase();
                        if kind.is_empty() {
                            kind = "verse".into();
                        }
                        cur = Some(Section {
                            kind,
                            label: attr(b"label"),
                            lines: Vec::new(),
                        });
                    }
                    b"line" => {
                        let t = text_of(&mut reader, b"line");
                        if let Some(s) = cur.as_mut() {
                            s.lines.push(t);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"section" => {
                if let Some(s) = cur.take()
                    && !s.lines.is_empty()
                {
                    song.sections.push(s);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML error in {}: {e}", path.display())),
            _ => {}
        }
    }
    finish(song, path)
}

/// Plain text: sections separated by blank lines; an optional header line like
/// `[Chorus]`, `Chorus:` or `Verse 2` names the section. A first line `Title: ...` sets the title.
pub fn parse_txt(text: &str, path: &Path) -> Result<Song, String> {
    let mut song = Song {
        path: path.to_path_buf(),
        ..Default::default()
    };
    let text = text
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    for block in text.split("\n\n").map(str::trim).filter(|b| !b.is_empty()) {
        let mut lines: Vec<&str> = block
            .lines()
            .map(str::trim_end)
            .filter(|l| !l.trim().is_empty())
            .collect();
        if let Some(first) = lines.first().copied() {
            for (key, field) in [
                ("title:", 0),
                ("author:", 1),
                ("copyright:", 2),
                ("ccli:", 3),
            ] {
                if first.to_lowercase().starts_with(key) && lines.len() == 1 {
                    let v = first[key.len()..].trim().to_string();
                    match field {
                        0 => song.title = v,
                        1 => song.author = v,
                        2 => song.copyright = v,
                        _ => song.ccli = v,
                    }
                    lines.clear();
                }
            }
        }
        if lines.is_empty() {
            continue;
        }
        let mut section = Section {
            kind: "verse".into(),
            ..Default::default()
        };
        if let Some(first) = lines.first() {
            let h = first
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim_end_matches(':')
                .trim();
            let kind = section_type_for_label(h);
            let is_header =
                (first.starts_with('[') || first.ends_with(':') || h.chars().count() <= 16)
                    && SECTION_TYPES.iter().any(|t| t.to_lowercase() == kind);
            if is_header && lines.len() > 1 {
                section.kind = kind;
                if h.chars().any(|c| c.is_ascii_digit()) || section.kind != "verse" {
                    section.label = String::new();
                }
                lines.remove(0);
            }
        }
        section.lines = lines.into_iter().map(str::to_string).collect();
        song.sections.push(section);
    }
    finish(song, path)
}

fn finish(mut song: Song, path: &Path) -> Result<Song, String> {
    if song.title.trim().is_empty() {
        song.title = path
            .file_stem()
            .map(|s| s.to_string_lossy().replace('_', " "))
            .unwrap_or_default();
    }
    if song.sections.is_empty() {
        return Err(format!("No content found in: {}", path.display()));
    }
    if !song.medley {
        song.medley = song.sections.iter().all(|s| s.label.contains(" \u{2014} "));
    }
    Ok(song)
}

pub fn load(path: &Path) -> Result<Song, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("Cannot open file {}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let is_xml = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("xml"))
        || text.trim_start().starts_with('<');
    if is_xml {
        parse_xml(&text, path)
    } else {
        parse_txt(&text, path)
    }
}

pub fn to_xml(song: &Song) -> String {
    fn esc(s: &str) -> String {
        quick_xml::escape::escape(s).into_owned()
    }
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str(if song.medley {
        "<song medley=\"true\">\n"
    } else {
        "<song>\n"
    });
    for (tag, v) in [
        ("title", &song.title),
        ("author", &song.author),
        ("copyright", &song.copyright),
        ("ccli", &song.ccli),
    ] {
        out.push_str(&format!("    <{tag}>{}</{tag}>\n", esc(v)));
    }
    out.push_str("    <lyrics>\n");
    for s in &song.sections {
        if s.label.is_empty() {
            out.push_str(&format!("        <section type=\"{}\">\n", esc(&s.kind)));
        } else {
            out.push_str(&format!(
                "        <section type=\"{}\" label=\"{}\">\n",
                esc(&s.kind),
                esc(&s.label)
            ));
        }
        for l in &s.lines {
            out.push_str(&format!("            <line>{}</line>\n", esc(l)));
        }
        out.push_str("        </section>\n");
    }
    out.push_str("    </lyrics>\n</song>\n");
    out
}

/// Lyrics as typed in the song editor: blank lines between sections, optional `[Label]`
/// markers that apply until the next marker.
pub fn editor_text(song: &Song) -> String {
    let labeled = song
        .sections
        .iter()
        .any(|s| !s.label.is_empty() || s.kind != "verse");
    let mut out = String::new();
    let mut previous = String::new();
    for s in &song.sections {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        if labeled {
            let marker = if !s.label.is_empty() {
                format!("[{}]", s.label)
            } else if s.kind != "verse" {
                format!("[{}]", display_name_for_type(&s.kind))
            } else {
                "[Verse]".to_string()
            };
            if marker != previous {
                out.push_str(&marker);
                out.push('\n');
            }
            previous = marker;
        }
        out.push_str(&s.text());
    }
    out
}

fn marker_of(line: &str) -> Option<&str> {
    let t = line.trim();
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    (!inner.is_empty() && !inner.contains(']')).then_some(inner.trim())
}

/// Sections from song-editor text (see [`editor_text`]).
pub fn parse_editor_text(text: &str) -> Vec<Section> {
    let text = text.replace("\r\n", "\n");
    let mut paragraphs: Vec<Vec<&str>> = vec![Vec::new()];
    for line in text.lines() {
        if line.trim().is_empty() {
            if paragraphs.last().is_some_and(|p| !p.is_empty()) {
                paragraphs.push(Vec::new());
            }
        } else if let Some(p) = paragraphs.last_mut() {
            p.push(line);
        }
    }
    let (mut kind, mut label, mut verse) = ("verse".to_string(), String::new(), 0u32);
    let mut out = Vec::new();
    for mut lines in paragraphs {
        if let Some(m) = lines.first().and_then(|l| marker_of(l)) {
            label = m.to_string();
            kind = section_type_for_label(&label).replace(' ', "-");
            if kind.is_empty() {
                kind = "verse".into();
            }
            if kind == "verse" {
                verse += 1;
                let suffix = label
                    .find(" \u{2014} ")
                    .map(|i| label[i..].to_string())
                    .unwrap_or_default();
                label = format!("Verse {verse}{suffix}");
            }
            lines.remove(0);
        }
        if lines.is_empty() {
            continue;
        }
        out.push(Section {
            kind: kind.clone(),
            label: label.clone(),
            lines: lines.iter().map(|l| l.trim_end().to_string()).collect(),
        });
    }
    out
}

/// A file name for a song title, without the extension.
pub fn file_base_name(title: &str) -> String {
    let mut s: String = title
        .trim()
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let s = s.trim().trim_end_matches('.').to_string();
    if s.is_empty() { "Song".into() } else { s }
}

/// Write `song` as XML into `dir`, replacing `old` (the file it was loaded from) if given.
pub fn save(song: &Song, dir: &Path, old: Option<&Path>) -> Result<PathBuf, String> {
    let path = dir.join(format!("{}.xml", file_base_name(&song.title)));
    let same = old.is_some_and(|o| o == path);
    if !same && path.exists() {
        return Err(format!(
            "A song file named \"{}\" already exists.",
            path.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        ));
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(&path, to_xml(song))
        .map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    if let Some(o) = old.filter(|_| !same) {
        let _ = std::fs::remove_file(o);
    }
    Ok(path)
}

#[derive(Default)]
pub struct SongLibrary {
    pub songs: BTreeMap<String, Song>,
    pub errors: Vec<String>,
}

impl SongLibrary {
    pub fn load_dir(dir: &Path) -> SongLibrary {
        let mut lib = SongLibrary::default();
        let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("xml") || e.eq_ignore_ascii_case("txt"))
            })
            .collect();
        files.sort();
        for f in files {
            match load(&f) {
                Ok(s) => {
                    lib.songs.insert(s.title.clone(), s);
                }
                Err(e) => lib.errors.push(e),
            }
        }
        lib
    }

    /// Titles sorted case-insensitively.
    pub fn titles(&self) -> Vec<String> {
        let mut t: Vec<String> = self.songs.keys().cloned().collect();
        t.sort_by_key(|s| s.to_lowercase());
        t
    }

    pub fn get(&self, title: &str) -> Option<&Song> {
        self.songs.get(title).or_else(|| {
            self.songs
                .values()
                .find(|s| s.title.eq_ignore_ascii_case(title))
        })
    }

    pub fn search(&self, q: &str) -> Vec<String> {
        let q = q.trim().to_lowercase();
        if q.is_empty() {
            return self.titles();
        }
        let mut title_hits = Vec::new();
        let mut lyric_hits = Vec::new();
        for s in self.songs.values() {
            if s.title.to_lowercase().contains(&q) || s.author.to_lowercase().contains(&q) {
                title_hits.push(s.title.clone());
            } else if s.full_text().to_lowercase().contains(&q) {
                lyric_hits.push(s.title.clone());
            }
        }
        title_hits.sort_by_key(|s| s.to_lowercase());
        lyric_hits.sort_by_key(|s| s.to_lowercase());
        title_hits.extend(lyric_hits);
        title_hits
    }
}

/// Copies the bundled sample songs into the user's song folder when it is empty.
pub fn seed_songs(dir: &Path) {
    let empty = std::fs::read_dir(dir)
        .map(|mut d| d.next().is_none())
        .unwrap_or(true);
    if !empty {
        return;
    }
    let bundled = crate::paths::resource_dir().join("songs");
    for e in std::fs::read_dir(bundled).into_iter().flatten().flatten() {
        let _ = std::fs::copy(e.path(), dir.join(e.file_name()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_and_txt() {
        let s = parse_xml(
            r#"<song><title>T &amp; U</title><lyrics><section type="chorus"><line>a</line></section><section><line>b</line></section><section><line>c</line></section></lyrics></song>"#,
            Path::new("x.xml"),
        )
        .unwrap();
        assert_eq!(s.title, "T & U");
        assert_eq!(s.display_label(0), "Chorus");
        assert_eq!(s.display_label(2), "Verse 2");
        let back = parse_xml(&to_xml(&s), Path::new("x.xml")).unwrap();
        assert_eq!(back.sections, s.sections);
        let t = parse_txt(
            "Line one\nLine two\n\n[Chorus]\nSing\n\n\nthird",
            Path::new("Amazing_Grace.txt"),
        )
        .unwrap();
        assert_eq!(t.title, "Amazing Grace");
        assert_eq!(t.sections.len(), 3);
        assert_eq!(t.sections[1].kind, "chorus");
        assert_eq!(section_type_for_label("Chorus 2 \u{2014} Oceans"), "chorus");
    }
}

#[cfg(test)]
mod editor_tests {
    use super::*;

    #[test]
    fn editor_round_trip() {
        let secs = parse_editor_text("[Verse]\na\nb\n\n[Chorus]\nc\n\nd\n\n\n[Verse]\ne");
        let kinds: Vec<_> = secs
            .iter()
            .map(|s| (s.kind.as_str(), s.label.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                ("verse", "Verse 1"),
                ("chorus", "Chorus"),
                ("chorus", "Chorus"),
                ("verse", "Verse 2")
            ]
        );
        let song = Song {
            title: "T".into(),
            sections: secs.clone(),
            ..Default::default()
        };
        assert_eq!(parse_editor_text(&editor_text(&song)), secs);
        let plain = parse_editor_text("x\ny\n\nz");
        assert_eq!(plain.len(), 2);
        assert!(
            plain
                .iter()
                .all(|s| s.kind == "verse" && s.label.is_empty())
        );
        assert_eq!(file_base_name(" a/b:c? "), "a_b_c_");
        assert_eq!(file_base_name("///"), "_");
        assert_eq!(file_base_name(""), "Song");
    }
}
