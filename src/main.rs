//! SimplePresenter: Bible and lyrics projection.
#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod audio;
mod bible;
mod browser;
mod canvas;
mod displays;
mod fonts;
mod media;
mod offscreen;
mod paths;
mod player;
mod playlist;
mod ppt;
mod server;
mod settings;
mod settings_ui;
mod songs;
mod theme;
mod update;

fn main() -> eframe::Result {
    env_logger::init();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--youtube-player") {
        return player::run(&args[2..]);
    }
    let icon = std::fs::read(paths::resource_dir().join("Logo.png"))
        .ok()
        .and_then(|b| image::load_from_memory(&b).ok())
        .map(|i| {
            let i = i
                .resize(256, 256, image::imageops::FilterType::Triangle)
                .to_rgba8();
            egui::IconData {
                width: i.width(),
                height: i.height(),
                rgba: i.into_raw(),
            }
        });
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("SimplePresenter")
        .with_inner_size([1400.0, 860.0])
        .with_min_inner_size([900.0, 600.0]);
    if let Some(icon) = icon {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "SimplePresenter",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
