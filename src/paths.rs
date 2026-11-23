//! Application folders.

use std::path::PathBuf;

/// Per-user writable data folder (Bibles, songs, services, media, settings).
pub fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("com", "psnact", "SimplePresenter")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("SimplePresenterData"))
}

/// Folder with bundled read-only resources (fonts, default Bibles, logo, remote.html).
pub fn resource_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SIMPLEPRESENTER_RESOURCES") {
        return PathBuf::from(dir);
    }
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
    for cand in [
        exe_dir.join("resources"),
        exe_dir.join("../Resources/resources"),
        exe_dir.join("../../resources"),
        PathBuf::from("resources"),
    ] {
        if cand.join("Logo.png").is_file() {
            return cand;
        }
    }
    exe_dir.join("resources")
}

pub fn songs_dir() -> PathBuf {
    sub("songs")
}

pub fn services_dir() -> PathBuf {
    sub("services")
}

pub fn media_dir() -> PathBuf {
    sub("media")
}

fn sub(name: &str) -> PathBuf {
    let p = data_dir().join(name);
    let _ = std::fs::create_dir_all(&p);
    p
}
