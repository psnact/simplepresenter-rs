//! Update check against the same `update.json` format the Qt app used.

use std::sync::mpsc::{self, Receiver};

pub const UPDATE_URL: &str =
    "https://raw.githubusercontent.com/psnact/SimplePresenter/main/update.json";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub download_url: String,
    pub notes: String,
}

/// `Ok(Some)` when a newer version is published, `Ok(None)` when up to date.
pub type Outcome = Result<Option<Release>, String>;

fn version_parts(v: &str) -> Vec<u64> {
    v.trim()
        .trim_start_matches(['v', 'V'])
        .split('.')
        .map(|p| {
            p.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    let (a, b) = (version_parts(latest), version_parts(current));
    let n = a.len().max(b.len());
    let at = |v: &[u64], i: usize| v.get(i).copied().unwrap_or(0);
    (0..n)
        .map(|i| at(&a, i).cmp(&at(&b, i)))
        .find(|o| o.is_ne())
        == Some(std::cmp::Ordering::Greater)
}

pub fn parse(json: &str, current: &str) -> Outcome {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|_| "Invalid update information format".to_string())?;
    let field = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let version = field("latest_version");
    if version.is_empty() {
        return Err("Missing version information in update file".into());
    }
    Ok(is_newer(&version, current).then(|| Release {
        version,
        download_url: field("download_url"),
        notes: field("release_notes"),
    }))
}

/// Fetch and compare in the background; `ctx` is repainted when the answer arrives.
pub fn check(ctx: egui::Context) -> Receiver<Outcome> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let result = ureq::get(UPDATE_URL)
            .call()
            .map_err(|e| e.to_string())
            .and_then(|mut r| {
                r.body_mut()
                    .with_config()
                    .limit(1 << 20)
                    .read_to_string()
                    .map_err(|e| e.to_string())
            })
            .and_then(|body| parse(&body, VERSION));
        let _ = tx.send(result);
        ctx.request_repaint();
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("1.0.1", "1.0.0"));
        assert!(is_newer("2.1", "2.0.9"));
        assert!(!is_newer("1.0.1", "2.0.0"));
        assert!(!is_newer("2.0.0", "2.0"));
        let r = parse(
            r#"{"latest_version":"3.0.0","download_url":"u","release_notes":"n"}"#,
            "2.0.0",
        );
        assert_eq!(
            r,
            Ok(Some(Release {
                version: "3.0.0".into(),
                download_url: "u".into(),
                notes: "n".into()
            }))
        );
        assert_eq!(parse(r#"{"latest_version":"1.0.1"}"#, "2.0.0"), Ok(None));
        assert!(parse("nope", "2.0.0").is_err());
    }
}
