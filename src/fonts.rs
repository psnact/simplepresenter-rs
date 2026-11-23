//! Registers the bundled OFL fonts with egui, one family per typeface (plus "<Family> Bold").

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{FontData, FontDefinitions, FontFamily};

/// "LiberationSans-Bold" -> ("Liberation Sans", bold). Italic faces are skipped.
fn family_of(stem: &str) -> Option<(String, bool)> {
    let base = stem.split(['[', '-', '_']).next().unwrap_or(stem);
    let style = stem.to_lowercase();
    if style.contains("italic") {
        return None;
    }
    let bold = style.ends_with("-bold") || style.ends_with("_bold");
    let regular_like =
        bold || !stem.contains('-') || style.ends_with("-regular") || stem.contains('[');
    if !regular_like {
        return None;
    }
    let mut name = String::new();
    let mut prev_lower = false;
    for c in base.chars() {
        if c.is_uppercase() && prev_lower {
            name.push(' ');
        }
        prev_lower = c.is_lowercase() || c.is_ascii_digit();
        name.push(c);
    }
    Some((name, bold))
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 4 {
        return;
    }
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out, depth + 1);
        } else if p
            .extension()
            .is_some_and(|x| x.eq_ignore_ascii_case("ttf") || x.eq_ignore_ascii_case("otf"))
        {
            out.push(p);
        }
    }
}

/// A system font face: the file and index inside it (for .ttc).
#[derive(Clone)]
struct SysFace {
    path: PathBuf,
    index: u32,
    weight: f32,
}

#[derive(Clone, Default)]
struct SysFamily {
    regular: Option<SysFace>,
    bold: Option<SysFace>,
}

#[derive(Clone)]
pub struct FontSet {
    /// Typeface names for the font pickers (bundled + installed on the PC).
    pub families: Vec<String>,
    /// Every registered egui family key (includes "<Family> Bold").
    pub keys: BTreeSet<String>,
    /// The definitions registered with egui; grown as system fonts load.
    defs: FontDefinitions,
    /// Installed fonts from fontdb, keyed by family name; loaded on demand.
    sys: BTreeMap<String, SysFamily>,
}

impl FontSet {
    /// Registers an installed font with egui the first time it's picked.
    pub fn ensure(&mut self, ctx: &egui::Context, family: &str) {
        if family.is_empty() || self.keys.contains(family) {
            return;
        }
        let Some(sys) = self.sys.get(family) else {
            return;
        };
        let mut loaded = false;
        for (key, face) in [
            (family.to_string(), &sys.regular),
            (format!("{family} Bold"), &sys.bold),
        ] {
            let Some(face) = face else { continue };
            let Ok(bytes) = std::fs::read(&face.path) else {
                continue;
            };
            let mut data = FontData::from_owned(bytes);
            data.index = face.index;
            self.defs.font_data.insert(key.clone(), Arc::new(data));
            let chain = vec![key.clone(), "Inter".to_string(), "Ubuntu-Light".to_string()];
            self.defs
                .families
                .insert(FontFamily::Name(key.clone().into()), chain);
            self.keys.insert(key);
            loaded = true;
        }
        if loaded {
            ctx.set_fonts(self.defs.clone());
        }
    }
}

impl FontSet {
    /// The egui family for a style, falling back to the default proportional font.
    pub fn family(&self, name: &str, bold: bool) -> FontFamily {
        let bold_key = format!("{name} Bold");
        if bold && self.keys.contains(&bold_key) {
            return FontFamily::Name(bold_key.into());
        }
        if self.keys.contains(name) {
            FontFamily::Name(name.to_string().into())
        } else {
            FontFamily::Proportional
        }
    }
}

/// Installs fonts and returns what is available for projection text.
pub fn install(ctx: &egui::Context) -> FontSet {
    let mut defs = FontDefinitions::default();
    let mut files = Vec::new();
    collect(&crate::paths::resource_dir().join("fonts"), &mut files, 0);
    files.sort();
    let fallbacks: Vec<String> = defs
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mut names = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for f in files {
        let Some(stem) = f.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        let Some((family, bold)) = family_of(&stem) else {
            continue;
        };
        let key = if bold {
            format!("{family} Bold")
        } else {
            family.clone()
        };
        if defs
            .families
            .contains_key(&FontFamily::Name(key.clone().into()))
        {
            continue;
        }
        let Ok(bytes) = std::fs::read(&f) else {
            continue;
        };
        defs.font_data
            .insert(key.clone(), Arc::new(FontData::from_owned(bytes)));
        let mut chain = vec![key.clone()];
        chain.extend(fallbacks.iter().cloned());
        defs.families
            .insert(FontFamily::Name(key.clone().into()), chain);
        keys.insert(key);
        names.insert(family);
    }
    if keys.contains("Inter")
        && let Some(p) = defs.families.get_mut(&FontFamily::Proportional)
    {
        p.insert(0, "Inter".to_string());
    }
    // Installed fonts from the OS, for the pickers; loaded into egui on demand.
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut sys: BTreeMap<String, SysFamily> = BTreeMap::new();
    for face in db.faces() {
        let fontdb::Source::File(path) = &face.source else {
            continue;
        };
        let Some((family, _)) = face.families.first() else {
            continue;
        };
        let ent = sys.entry(family.clone()).or_default();
        let slot = if face.weight >= fontdb::Weight::SEMIBOLD {
            &mut ent.bold
        } else {
            &mut ent.regular
        };
        // Prefer the face whose weight is closest to its slot's target.
        let target = if face.weight >= fontdb::Weight::SEMIBOLD {
            700.0
        } else {
            400.0
        };
        let better = slot
            .as_ref()
            .is_none_or(|cur| (cur.weight - target).abs() > (face.weight.0 as f32 - target).abs());
        if better {
            *slot = Some(SysFace {
                path: path.clone(),
                index: face.index,
                weight: face.weight.0 as f32,
            });
        }
    }
    for f in sys.keys() {
        names.insert(f.clone());
    }
    let font_defs = defs.clone();
    ctx.set_fonts(defs);
    FontSet {
        families: names.into_iter().collect(),
        keys,
        defs: font_defs,
        sys,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn names() {
        assert_eq!(
            super::family_of("LiberationSans-Bold"),
            Some(("Liberation Sans".into(), true))
        );
        assert_eq!(
            super::family_of("Inter[opsz,wght]"),
            Some(("Inter".into(), false))
        );
        assert_eq!(
            super::family_of("Oswald-Regular"),
            Some(("Oswald".into(), false))
        );
        assert_eq!(super::family_of("Lato-Light"), None);
        assert_eq!(super::family_of("Lato-Italic"), None);
    }
}
