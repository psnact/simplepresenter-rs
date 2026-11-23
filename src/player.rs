//! `--youtube-player URL X Y W H fullscreen|window`: a borderless window on the projector screen
//! that plays a YouTube video in a native web view. Started and stopped by the main window.

use crate::browser::WebPane;

struct Player {
    url: String,
    pane: WebPane,
}

impl eframe::App for Player {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show(ui, |ui| {
                let rect = ui.max_rect();
                let url = self.url.clone();
                self.pane.show(frame, rect, ctx.pixels_per_point(), &url);
                if let Some(e) = &self.pane.error {
                    ui.colored_label(egui::Color32::LIGHT_RED, e);
                }
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

pub fn run(args: &[String]) -> eframe::Result {
    let url = args.first().cloned().unwrap_or_default();
    let num = |i: usize, d: f32| {
        args.get(i)
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(d)
    };
    let (x, y, w, h) = (
        num(1, 100.0),
        num(2, 100.0),
        num(3, 1280.0).max(160.0),
        num(4, 720.0).max(90.0),
    );
    let fullscreen = args.get(5).is_some_and(|a| a == "fullscreen");
    let viewport = egui::ViewportBuilder::default()
        .with_title("SimplePresenter YouTube")
        .with_position([x, y])
        .with_inner_size([w, h])
        .with_decorations(false)
        .with_fullscreen(fullscreen)
        .with_always_on_top();
    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };
    eframe::run_native(
        "SimplePresenter YouTube",
        options,
        Box::new(move |_| {
            Ok(Box::new(Player {
                url,
                pane: WebPane::default(),
            }))
        }),
    )
}
