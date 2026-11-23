//! Bible XML loading, reference parsing and full-text search.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use quick_xml::Reader;
use quick_xml::events::Event;
use regex::Regex;

#[derive(Clone, Debug, PartialEq)]
pub struct Verse {
    pub book: String,
    pub chapter: u32,
    pub verse: u32,
    pub text: String,
}

impl Verse {
    pub fn reference(&self) -> String {
        format!("{} {}:{}", self.book, self.chapter, self.verse)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reference {
    pub book: String,
    pub chapter: u32,
    pub start: u32,
    pub end: u32,
    /// True when only book + chapter were given.
    pub chapter_only: bool,
}

#[derive(Default)]
pub struct Bible {
    pub translation: String,
    pub acronym: String,
    /// Books in file order.
    pub books: Vec<Book>,
}

#[derive(Default, Clone)]
pub struct Book {
    pub name: String,
    pub chapters: BTreeMap<u32, BTreeMap<u32, String>>,
}

const ALIASES: &[(&str, &str)] = &[
    ("gen", "Genesis"),
    ("exo", "Exodus"),
    ("lev", "Leviticus"),
    ("num", "Numbers"),
    ("deu", "Deuteronomy"),
    ("jos", "Joshua"),
    ("jdg", "Judges"),
    ("rut", "Ruth"),
    ("1sa", "1 Samuel"),
    ("2sa", "2 Samuel"),
    ("1ki", "1 Kings"),
    ("2ki", "2 Kings"),
    ("1ch", "1 Chronicles"),
    ("2ch", "2 Chronicles"),
    ("ezr", "Ezra"),
    ("neh", "Nehemiah"),
    ("est", "Esther"),
    ("job", "Job"),
    ("psa", "Psalms"),
    ("pro", "Proverbs"),
    ("ecc", "Ecclesiastes"),
    ("sng", "Song of Solomon"),
    ("isa", "Isaiah"),
    ("jer", "Jeremiah"),
    ("lam", "Lamentations"),
    ("ezk", "Ezekiel"),
    ("dan", "Daniel"),
    ("hos", "Hosea"),
    ("jol", "Joel"),
    ("amo", "Amos"),
    ("oba", "Obadiah"),
    ("jon", "Jonah"),
    ("mic", "Micah"),
    ("nam", "Nahum"),
    ("hab", "Habakkuk"),
    ("zep", "Zephaniah"),
    ("hag", "Haggai"),
    ("zec", "Zechariah"),
    ("mal", "Malachi"),
    ("mat", "Matthew"),
    ("mrk", "Mark"),
    ("luk", "Luke"),
    ("jhn", "John"),
    ("act", "Acts"),
    ("rom", "Romans"),
    ("1co", "1 Corinthians"),
    ("2co", "2 Corinthians"),
    ("gal", "Galatians"),
    ("eph", "Ephesians"),
    ("php", "Philippians"),
    ("col", "Colossians"),
    ("1th", "1 Thessalonians"),
    ("2th", "2 Thessalonians"),
    ("1ti", "1 Timothy"),
    ("2ti", "2 Timothy"),
    ("tit", "Titus"),
    ("phm", "Philemon"),
    ("heb", "Hebrews"),
    ("jas", "James"),
    ("1pe", "1 Peter"),
    ("2pe", "2 Peter"),
    ("1jn", "1 John"),
    ("2jn", "2 John"),
    ("3jn", "3 John"),
    ("jud", "Jude"),
    ("rev", "Revelation"),
];

fn attr(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == name)
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

fn letters_upper(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphabetic())
        .flat_map(char::to_uppercase)
        .collect()
}

fn acronym_from_name(source: &str) -> Option<String> {
    if source.is_empty() {
        return None;
    }
    if let (Some(open), Some(close)) = (source.rfind('('), source.rfind(')'))
        && close > open + 1
    {
        let inside = letters_upper(&source[open + 1..close]);
        if !inside.is_empty() && inside.chars().count() <= 12 {
            return Some(inside);
        }
    }
    let last = source
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .last()?;
    let l = letters_upper(last);
    (2..=12).contains(&l.chars().count()).then_some(l)
}

impl Bible {
    pub fn load(path: &Path) -> Result<Bible, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot open file {}: {e}", path.display()))?;
        let base = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        Self::parse(&text, &base)
    }

    pub fn parse(xml: &str, file_base: &str) -> Result<Bible, String> {
        let mut reader = Reader::from_str(xml);
        let mut bible = Bible::default();
        let mut translation_code = String::new();
        let mut book_idx: Option<usize> = None;
        let mut chapter = 0u32;
        let mut verse_num: Option<u32> = None;
        let mut verse_text = String::new();
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    let name = e.name();
                    match name.as_ref() {
                        b"XMLBIBLE" | b"bible" => {
                            let biblename = attr(&e, b"biblename").unwrap_or_default();
                            translation_code = attr(&e, b"translation").unwrap_or_default();
                            bible.translation = if !biblename.is_empty() {
                                biblename
                            } else {
                                translation_code.clone()
                            };
                        }
                        b"BIBLEBOOK" | b"book" => {
                            let n = attr(&e, b"bname")
                                .filter(|s| !s.is_empty())
                                .or_else(|| attr(&e, b"name"))
                                .unwrap_or_default();
                            book_idx = None;
                            if !n.is_empty() {
                                if let Some(i) = bible.books.iter().position(|b| b.name == n) {
                                    book_idx = Some(i);
                                } else {
                                    bible.books.push(Book {
                                        name: n,
                                        chapters: BTreeMap::new(),
                                    });
                                    book_idx = Some(bible.books.len() - 1);
                                }
                            }
                        }
                        b"CHAPTER" | b"chapter" => {
                            chapter = attr(&e, b"cnumber")
                                .and_then(|v| v.trim().parse().ok())
                                .filter(|&n| n > 0)
                                .or_else(|| attr(&e, b"number").and_then(|v| v.trim().parse().ok()))
                                .unwrap_or(0);
                        }
                        b"VERS" | b"verse" => {
                            verse_num = attr(&e, b"vnumber")
                                .and_then(|v| v.trim().parse().ok())
                                .filter(|&n| n > 0)
                                .or_else(|| {
                                    attr(&e, b"number").and_then(|v| v.trim().parse().ok())
                                });
                            verse_text.clear();
                        }
                        _ => {}
                    }
                }
                Ok(Event::Text(t)) => {
                    if verse_num.is_some() {
                        let s = t.decode().map(|c| c.into_owned()).unwrap_or_default();
                        verse_text.push_str(
                            &quick_xml::escape::unescape(&s)
                                .map(|c| c.into_owned())
                                .unwrap_or(s),
                        );
                    }
                }
                Ok(Event::GeneralRef(r)) => {
                    if verse_num.is_some() {
                        let name = r.decode().map(|c| c.into_owned()).unwrap_or_default();
                        let ent = format!("&{name};");
                        verse_text.push_str(
                            &quick_xml::escape::unescape(&ent)
                                .map(|c| c.into_owned())
                                .unwrap_or_default(),
                        );
                    }
                }
                Ok(Event::CData(t)) => {
                    if verse_num.is_some() {
                        verse_text.push_str(&String::from_utf8_lossy(&t));
                    }
                }
                Ok(Event::End(e)) => {
                    if matches!(e.name().as_ref(), b"VERS" | b"verse") {
                        if let (Some(v), Some(bi)) = (verse_num.take(), book_idx)
                            && chapter > 0
                            && v > 0
                            && let Some(book) = bible.books.get_mut(bi)
                        {
                            book.chapters
                                .entry(chapter)
                                .or_default()
                                .insert(v, verse_text.trim().to_string());
                        }
                        verse_num = None;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(format!("XML parse error: {e}")),
                _ => {}
            }
        }
        if bible.translation.is_empty() {
            bible.translation = file_base.to_string();
        }
        let cand = letters_upper(translation_code.trim());
        bible.acronym = if !cand.is_empty() && cand.chars().count() <= 12 {
            cand
        } else {
            acronym_from_name(file_base)
                .or_else(|| acronym_from_name(&bible.translation))
                .unwrap_or_else(|| {
                    bible
                        .translation
                        .split_whitespace()
                        .filter_map(|w| w.chars().next())
                        .flat_map(char::to_uppercase)
                        .take(8)
                        .collect()
                })
        };
        if bible.acronym.is_empty() {
            bible.acronym = bible.translation.clone();
        }
        Ok(bible)
    }

    pub fn book_names(&self) -> Vec<String> {
        self.books.iter().map(|b| b.name.clone()).collect()
    }

    fn book(&self, name: &str) -> Option<&Book> {
        let n = self.normalize_book(name);
        self.books.iter().find(|b| b.name == n)
    }

    pub fn normalize_book(&self, name: &str) -> String {
        let lower = name
            .trim()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        if let Some(b) = self.books.iter().find(|b| b.name.to_lowercase() == lower) {
            return b.name.clone();
        }
        let has = |n: &str| self.books.iter().any(|b| b.name == n);
        let compact = lower.replace(' ', "");
        if let Some(b) = self
            .books
            .iter()
            .find(|b| b.name.to_lowercase().replace(' ', "") == compact)
        {
            return b.name.clone();
        }
        if let Some((_, full)) = ALIASES.iter().find(|(a, _)| *a == compact)
            && has(full)
        {
            return (*full).to_string();
        }
        let mut cands: Vec<String> = Vec::new();
        for b in &self.books {
            let lb = b.name.to_lowercase().replace(' ', "");
            if lb.starts_with(&compact) || compact.starts_with(&lb) {
                cands.push(b.name.clone());
            }
        }
        for (a, full) in ALIASES {
            if a.starts_with(&compact) && has(full) && !cands.iter().any(|c| c == full) {
                cands.push((*full).to_string());
            }
        }
        if cands.len() == 1 {
            return cands.remove(0);
        }
        name.to_string()
    }

    pub fn autocomplete_book(&self, partial: &str) -> Vec<String> {
        let lp = partial.to_lowercase();
        let mut out: Vec<String> = self
            .books
            .iter()
            .filter(|b| b.name.to_lowercase().starts_with(&lp))
            .map(|b| b.name.clone())
            .collect();
        for (a, full) in ALIASES {
            if a.starts_with(&lp)
                && self.books.iter().any(|b| b.name == *full)
                && !out.iter().any(|o| o == full)
            {
                out.push((*full).to_string());
            }
        }
        out
    }

    pub fn parse_reference(&self, reference: &str) -> Option<Reference> {
        let mut fixed = reference
            .trim()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if let Ok(re) = Regex::new(r"^(\d)([A-Za-z])") {
            fixed = re.replace(&fixed, "$1 $2").into_owned();
        }
        if let Ok(re) = Regex::new(r"([A-Za-z])(\d)") {
            fixed = re.replace_all(&fixed, "$1 $2").into_owned();
        }
        let re = Regex::new(r"^(.+?)\s+(\d+)(?:\s*[: ]\s*(\d+)(?:\s*-\s*(\d+))?)?$").ok()?;
        let c = re.captures(&fixed)?;
        let book = self.normalize_book(c.get(1)?.as_str().trim());
        let chapter: u32 = c.get(2)?.as_str().parse().ok()?;
        match c.get(3) {
            None => Some(Reference {
                book,
                chapter,
                start: 1,
                end: 1,
                chapter_only: true,
            }),
            Some(s) => {
                let start: u32 = s.as_str().parse().ok()?;
                let end = c
                    .get(4)
                    .and_then(|e| e.as_str().parse().ok())
                    .unwrap_or(start);
                Some(Reference {
                    book,
                    chapter,
                    start,
                    end,
                    chapter_only: false,
                })
            }
        }
    }

    pub fn verses(&self, book: &str, chapter: u32, start: u32, end: u32) -> Vec<Verse> {
        let Some(b) = self.book(book) else {
            return Vec::new();
        };
        let Some(ch) = b.chapters.get(&chapter) else {
            return Vec::new();
        };
        ch.range(start..=end.max(start))
            .map(|(&v, t)| Verse {
                book: b.name.clone(),
                chapter,
                verse: v,
                text: t.clone(),
            })
            .collect()
    }

    pub fn chapter(&self, book: &str, chapter: u32) -> Vec<Verse> {
        self.verses(book, chapter, 1, u32::MAX)
    }

    pub fn chapter_count(&self, book: &str) -> usize {
        self.book(book).map(|b| b.chapters.len()).unwrap_or(0)
    }

    /// The `n`th existing verse after (`n > 0`) or before (`n < 0`) verse `from`, staying in the chapter.
    pub fn neighbour_verse(&self, book: &str, chapter: u32, from: u32, n: i32) -> Option<u32> {
        let ch = self.book(book)?.chapters.get(&chapter)?;
        let steps = n.unsigned_abs() as usize;
        if n >= 0 {
            ch.range(from.saturating_add(1)..)
                .map(|(v, _)| *v)
                .take(steps.max(1))
                .last()
        } else {
            ch.range(..from).rev().map(|(v, _)| *v).take(steps).last()
        }
    }

    pub fn next_chapter(&self, book: &str, chapter: u32) -> Option<u32> {
        self.book(book)?
            .chapters
            .range(chapter.saturating_add(1)..)
            .find(|(_, v)| !v.is_empty())
            .map(|(c, _)| *c)
    }

    pub fn prev_chapter(&self, book: &str, chapter: u32) -> Option<u32> {
        self.book(book)?
            .chapters
            .range(..chapter)
            .rev()
            .find(|(_, v)| !v.is_empty())
            .map(|(c, _)| *c)
    }

    pub fn first_verse(&self, book: &str, chapter: u32) -> Option<u32> {
        self.book(book)?
            .chapters
            .get(&chapter)?
            .keys()
            .next()
            .copied()
    }

    pub fn last_verse(&self, book: &str, chapter: u32) -> Option<u32> {
        self.book(book)?
            .chapters
            .get(&chapter)?
            .keys()
            .next_back()
            .copied()
    }

    pub fn search(&self, query: &str, max_results: usize) -> Vec<Verse> {
        let mut results = Vec::new();
        let nq = normalize_for_search(query);
        if nq.is_empty() {
            return results;
        }
        let tokens: Vec<&str> = nq.split(' ').filter(|t| t.chars().count() >= 2).collect();
        let token_mode = tokens.len() >= 2;
        for b in &self.books {
            for (&c, verses) in &b.chapters {
                for (&v, text) in verses {
                    let nv = normalize_for_search(text);
                    let ok = if !token_mode {
                        nv.contains(&nq)
                    } else {
                        let words: Vec<&str> =
                            nv.split(' ').filter(|w| w.chars().count() >= 2).collect();
                        tokens.iter().all(|t| {
                            nv.contains(t)
                                || words.iter().any(|w| w.starts_with(t) || t.starts_with(w))
                                || (t.chars().count() >= 4
                                    && words.iter().any(|w| edit_distance_at_most(t, w, 1)))
                        })
                    };
                    if ok {
                        results.push(Verse {
                            book: b.name.clone(),
                            chapter: c,
                            verse: v,
                            text: text.clone(),
                        });
                        if results.len() >= max_results {
                            return results;
                        }
                    }
                }
            }
        }
        results
    }
}

fn normalize_for_search(s: &str) -> String {
    let lower = s.to_lowercase();
    let mapped: String = lower
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();
    mapped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn edit_distance_at_most(a: &str, b: &str, max: usize) -> bool {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return false;
    }
    if a == b {
        return true;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        let mut row_min = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            row_min = row_min.min(cur[j]);
        }
        if row_min > max {
            return false;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()] <= max
}

/// Folder holding Bible XML files.
pub fn bible_directory() -> PathBuf {
    let app = crate::paths::data_dir().join("Bible");
    let _ = std::fs::create_dir_all(&app);
    #[cfg(windows)]
    {
        let legacy = PathBuf::from("C:/SimplePresenter/Bible");
        if !has_xml(&app) && has_xml(&legacy) {
            return legacy;
        }
    }
    app
}

fn has_xml(dir: &Path) -> bool {
    list_xml(dir).next().is_some()
}

fn list_xml(dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xml")))
}

/// Available Bible files; seeds the folder from the bundled Bibles when empty.
/// Only files inside the Bible folder may be deleted (as in the original).
pub fn can_delete(path: &Path) -> bool {
    match (
        std::fs::canonicalize(path),
        std::fs::canonicalize(bible_directory()),
    ) {
        (Ok(file), Ok(dir)) => file.starts_with(&dir) && file.is_file(),
        _ => false,
    }
}

pub fn available_bibles() -> Vec<PathBuf> {
    let dir = bible_directory();
    // Seed bundled Bibles that are not in the data folder yet (never overwrite
    // the user's own copies, and add new bundled versions to existing installs).
    let bundled = crate::paths::resource_dir().join("Bible");
    for src in list_xml(&bundled) {
        if let Some(name) = src.file_name() {
            let dst = dir.join(name);
            if !dst.exists() {
                let _ = std::fs::copy(&src, dst);
            }
        }
    }
    let mut v: Vec<PathBuf> = list_xml(&dir).collect();
    v.sort();
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0"?><XMLBIBLE biblename="New King James Version (NKJV)">
<BIBLEBOOK bnumber="43" bname="John"><CHAPTER cnumber="3"><VERS vnumber="16">For God so loved the world</VERS><VERS vnumber="17">For God did not send &amp; condemn</VERS></CHAPTER></BIBLEBOOK>
<BIBLEBOOK bnumber="62" bname="1 John"><CHAPTER cnumber="1"><VERS vnumber="1">That which was from the beginning</VERS></CHAPTER></BIBLEBOOK>
<BIBLEBOOK bnumber="19" bname="Psalm"><CHAPTER cnumber="23"><VERS vnumber="1">The LORD is my shepherd</VERS></CHAPTER></BIBLEBOOK>
</XMLBIBLE>"#;

    #[test]
    fn parses_and_finds() {
        let b = Bible::parse(XML, "NKJV").unwrap();
        assert_eq!(b.acronym, "NKJV");
        assert_eq!(b.verses("john", 3, 16, 17).len(), 2);
        assert!(b.verses("John", 3, 17, 17)[0].text.contains('&'));
        let r = b.parse_reference("jhn3:16-17").unwrap();
        assert_eq!(
            (r.book.as_str(), r.chapter, r.start, r.end),
            ("John", 3, 16, 17)
        );
        assert_eq!(b.parse_reference("1jn 1").unwrap().book, "1 John");
        assert_eq!(b.parse_reference("psalms 23:1").unwrap().book, "Psalm");
        assert_eq!(b.search("loved world", 10).len(), 1);
        assert_eq!(b.search("shepherd", 10).len(), 1);
    }

    #[test]
    fn neighbours_skip_missing_verses() {
        let b = Bible::parse(XML, "NKJV").unwrap();
        assert_eq!(b.neighbour_verse("John", 3, 16, 1), Some(17));
        assert_eq!(b.neighbour_verse("John", 3, 17, 1), None);
        assert_eq!(b.neighbour_verse("John", 3, 17, -1), Some(16));
        assert_eq!(b.neighbour_verse("John", 3, 16, -1), None);
        assert_eq!(b.neighbour_verse("John", 3, 1, 2), Some(17));
        assert_eq!(
            (b.first_verse("John", 3), b.last_verse("John", 3)),
            (Some(16), Some(17))
        );
        assert_eq!(
            (b.next_chapter("John", 1), b.next_chapter("John", 3)),
            (Some(3), None)
        );
        assert_eq!(
            (b.prev_chapter("John", 9), b.prev_chapter("John", 3)),
            (Some(3), None)
        );
    }

    #[test]
    fn legacy_format() {
        let b = Bible::parse(r#"<bible translation="KJV"><book name="Genesis"><chapter number="1"><verse number="1">In the beginning</verse></chapter></book></bible>"#, "x").unwrap();
        assert_eq!(b.acronym, "KJV");
        assert_eq!(b.verses("Gen", 1, 1, 1).len(), 1);
    }
}
