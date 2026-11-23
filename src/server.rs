//! Overlay server for OBS and the phone remote. Keeps the Qt app's URLs (`/overlay`, `/data`,
//! `/remote`, `/projection.jpg`, `/projection.mjpg`, `/media`) and its WebSocket protocol on the
//! next port up (the remote page connects to `ws://host:8081`).

use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, RawQuery, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;

use crate::settings::{HAlign, OverlayConfig, RefPos, Rgba, TextStyle};

/// What `/data` returns (same fields as the Qt app).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub reference: String,
    pub text: String,
    pub text_html: String,
    pub has_media: bool,
    pub media_is_video: bool,
    pub media_timestamp: i64,
    pub media_playing: bool,
    pub media_position_ms: i64,
    pub refresh_timestamp: i64,
    #[serde(rename = "hasYouTube")]
    pub has_youtube: bool,
    #[serde(rename = "youTubeUrl")]
    pub youtube_url: String,
    pub notes_visible: bool,
    pub notes_html: String,
    pub notes_image_timestamp: i64,
    pub blackout: bool,
    pub media_fullscreen: bool,
    pub style_version: u64,
}

#[derive(Default)]
pub struct Shared {
    pub data: Data,
    pub media_path: String,
    pub config: OverlayConfig,
    pub frame_jpeg: Arc<Vec<u8>>,
    /// Bumped whenever `frame_jpeg` is replaced.
    pub frame_seq: u64,
    /// Current PowerPoint deck's slides (JPEG), served at `/slide/N.png` (1-based).
    pub slides: Vec<Arc<Vec<u8>>>,
    /// When a client last asked for a projection frame (seconds since start).
    pub frame_wanted: f64,
    started: Option<std::time::Instant>,
}

impl Shared {
    fn now(&self) -> f64 {
        self.started
            .map(|s| s.elapsed().as_secs_f64())
            .unwrap_or(0.0)
    }
    /// Whether anyone watched the projection stream in the last few seconds.
    pub fn frames_wanted(&self) -> bool {
        self.now() - self.frame_wanted < 3.0
    }
}

#[derive(Clone)]
struct AppState {
    shared: Arc<Mutex<Shared>>,
    tx: broadcast::Sender<String>,
    cmd_tx: mpsc::Sender<Value>,
    resources: PathBuf,
    remote_state: Arc<Mutex<Value>>,
    ctx: egui::Context,
}

pub struct Server {
    shared: Arc<Mutex<Shared>>,
    remote_state: Arc<Mutex<Value>>,
    tx: broadcast::Sender<String>,
    cmds: mpsc::Receiver<Value>,
    pub port: u16,
    _rt: tokio::runtime::Runtime,
}

impl Server {
    pub fn start(port: u16, resources: PathBuf, ctx: egui::Context) -> Result<Server, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let shared = Arc::new(Mutex::new(Shared {
            started: Some(std::time::Instant::now()),
            ..Shared::default()
        }));
        let (tx, _) = broadcast::channel(256);
        let (cmd_tx, cmds) = mpsc::channel();
        let remote_state = Arc::new(Mutex::new(serde_json::json!({ "type": "remoteState" })));
        let state = AppState {
            shared: shared.clone(),
            tx: tx.clone(),
            cmd_tx,
            resources,
            remote_state: remote_state.clone(),
            ctx,
        };
        let http = Router::new()
            .route("/", get(overlay))
            .route("/overlay", get(overlay))
            .route("/overlay.css", get(overlay_css))
            .route("/data", get(data))
            .route("/remote", get(remote))
            .route("/logo.png", get(logo))
            .route("/favicon.ico", get(logo))
            .route("/projection", get(projection_page))
            .route("/projection.jpg", get(projection_jpg))
            .route("/projection.mjpg", get(projection_mjpg))
            .route("/projection.mjpeg", get(projection_mjpg))
            .route("/media", get(media))
            .route("/slide/{file}", get(slide))
            .route("/ws", get(ws))
            .with_state(state.clone());
        let wsr = Router::new()
            .route("/", get(ws))
            .route("/ws", get(ws))
            .with_state(state);
        let ws_port = port.checked_add(1).ok_or("port out of range")?;
        let (a, b) = rt.block_on(async {
            let a = tokio::net::TcpListener::bind(("0.0.0.0", port))
                .await
                .map_err(|e| format!("port {port}: {e}"))?;
            let b = tokio::net::TcpListener::bind(("0.0.0.0", ws_port))
                .await
                .map_err(|e| format!("port {ws_port}: {e}"))?;
            Ok::<_, String>((a, b))
        })?;
        rt.spawn(async move {
            if let Err(e) = axum::serve(a, http).await {
                log::warn!("overlay server: {e}");
            }
        });
        rt.spawn(async move {
            if let Err(e) = axum::serve(b, wsr).await {
                log::warn!("overlay websocket: {e}");
            }
        });
        Ok(Server {
            shared,
            remote_state,
            tx,
            cmds,
            port,
            _rt: rt,
        })
    }

    pub fn update(&self, f: impl FnOnce(&mut Shared)) {
        if let Ok(mut s) = self.shared.lock() {
            f(&mut s);
        }
    }

    pub fn frames_wanted(&self) -> bool {
        self.shared
            .lock()
            .map(|s| s.frames_wanted())
            .unwrap_or(false)
    }

    /// Send a message to every WebSocket client.
    pub fn send(&self, msg: &Value) {
        let _ = self.tx.send(msg.to_string());
    }

    /// Store and broadcast the remote's state (`type: remoteState`) when it changed.
    pub fn set_remote_state(&self, state: Value) {
        let changed = match self.remote_state.lock() {
            Ok(mut s) if *s != state => {
                *s = state.clone();
                true
            }
            _ => false,
        };
        if changed {
            self.send(&state);
        }
    }

    /// Remote commands received since the last call.
    pub fn commands(&self) -> Vec<Value> {
        self.cmds.try_iter().collect()
    }
}

fn not_found() -> Response {
    (StatusCode::NOT_FOUND, "Not found").into_response()
}

fn portrait(q: &Option<String>) -> bool {
    q.as_deref().is_some_and(|q| q.contains("portrait"))
}

async fn overlay(State(st): State<AppState>, RawQuery(q): RawQuery) -> Response {
    let Ok(html) = std::fs::read_to_string(st.resources.join("overlay.html")) else {
        return not_found();
    };
    let html = html.replace("__PORTRAIT__", if portrait(&q) { "?portrait" } else { "" });
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        html,
    )
        .into_response()
}

async fn overlay_css(State(st): State<AppState>, RawQuery(q): RawQuery) -> Response {
    let cfg = st
        .shared
        .lock()
        .map(|s| s.config.clone())
        .unwrap_or_default();
    (
        [
            (header::CONTENT_TYPE, "text/css"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        css(&cfg, portrait(&q)),
    )
        .into_response()
}

async fn data(State(st): State<AppState>) -> Response {
    let d = st.shared.lock().map(|s| s.data.clone()).unwrap_or_default();
    (
        [
            (header::CONTENT_TYPE, "application/json"),
            (header::CACHE_CONTROL, "no-store"),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        ],
        serde_json::to_string(&d).unwrap_or_default(),
    )
        .into_response()
}

async fn remote(State(st): State<AppState>) -> Response {
    match std::fs::read_to_string(st.resources.join("remote.html")) {
        Ok(html) => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response(),
        Err(_) => not_found(),
    }
}

async fn logo(State(st): State<AppState>) -> Response {
    match std::fs::read(st.resources.join("Logo.png")) {
        Ok(b) => ([(header::CONTENT_TYPE, "image/png")], b).into_response(),
        Err(_) => not_found(),
    }
}

async fn projection_page() -> Response {
    let html = "<!DOCTYPE html><html><head><meta charset=\"UTF-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\"><title>Projection</title><style>html,body{margin:0;padding:0;background:#000;height:100%;width:100%;overflow:hidden}#img{width:100vw;height:100vh;object-fit:contain;display:block}</style></head><body><img id=\"img\" src=\"/projection.mjpg\" alt=\"Projection\"></body></html>";
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], html).into_response()
}

fn take_frame(st: &AppState) -> Arc<Vec<u8>> {
    st.ctx.request_repaint();
    match st.shared.lock() {
        Ok(mut s) => {
            s.frame_wanted = s.now();
            s.frame_jpeg.clone()
        }
        Err(_) => Arc::new(Vec::new()),
    }
}

async fn projection_jpg(State(st): State<AppState>) -> Response {
    // After an idle spell the cached frame is old: wait briefly for a fresh one, else serve the last.
    let (idle, seq) = st
        .shared
        .lock()
        .map(|s| (!s.frames_wanted(), s.frame_seq))
        .unwrap_or((false, 0));
    let mut f = take_frame(&st);
    for _ in 0..20 {
        let fresh = st.shared.lock().map(|s| s.frame_seq != seq).unwrap_or(true);
        if !f.is_empty() && (!idle || fresh) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        f = take_frame(&st);
    }
    if f.is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE, "No frame yet").into_response();
    }
    (
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        f.as_ref().clone(),
    )
        .into_response()
}

async fn projection_mjpg(State(st): State<AppState>) -> Response {
    let stream =
        futures_util::stream::unfold((st, None::<Arc<Vec<u8>>>), |(st, prev)| async move {
            loop {
                tokio::time::sleep(Duration::from_millis(100)).await;
                let f = take_frame(&st);
                if f.is_empty() || prev.as_ref().is_some_and(|p| Arc::ptr_eq(p, &f)) {
                    continue;
                }
                let mut part = format!(
                    "--frame\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    f.len()
                )
                .into_bytes();
                part.extend_from_slice(&f);
                part.extend_from_slice(b"\r\n");
                return Some((Ok::<_, Infallible>(part), (st, Some(f))));
            }
        });
    (
        [
            (
                header::CONTENT_TYPE,
                "multipart/x-mixed-replace; boundary=frame",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

async fn media(State(st): State<AppState>) -> Response {
    let path = st
        .shared
        .lock()
        .map(|s| s.media_path.clone())
        .unwrap_or_default();
    if path.is_empty() {
        return not_found();
    }
    let ext = std::path::Path::new(&path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mime = match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "webm" => "video/webm",
        "mkv" => "video/x-matroska",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    };
    match tokio::fs::read(&path).await {
        Ok(b) => ([(header::CONTENT_TYPE, mime)], b).into_response(),
        Err(_) => not_found(),
    }
}

async fn slide(State(st): State<AppState>, Path(file): Path<String>) -> Response {
    let n: usize = file
        .split('.')
        .next()
        .and_then(|n| n.parse().ok())
        .unwrap_or(0);
    let jpeg = st
        .shared
        .lock()
        .ok()
        .and_then(|s| s.slides.get(n.wrapping_sub(1)).cloned())
        .filter(|j| !j.is_empty());
    match jpeg {
        Some(j) => ([(header::CONTENT_TYPE, "image/jpeg")], j.as_ref().clone()).into_response(),
        None => not_found(),
    }
}

async fn ws(State(st): State<AppState>, up: WebSocketUpgrade) -> Response {
    up.on_upgrade(move |socket| client(st, socket))
}

async fn client(st: AppState, socket: WebSocket) {
    let (mut sink, mut stream) = socket.split();
    let hello = st
        .remote_state
        .lock()
        .map(|s| s.to_string())
        .unwrap_or_default();
    if sink.send(Message::Text(hello.into())).await.is_err() {
        return;
    }
    let mut rx = st.tx.subscribe();
    let forward = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(m) => {
                    if sink.send(Message::Text(m.into())).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
    while let Some(Ok(msg)) = stream.next().await {
        if let Message::Text(t) = msg
            && let Ok(v) = serde_json::from_str::<Value>(t.as_str())
            && v.get("type").and_then(Value::as_str) == Some("remote")
            && v.get("action")
                .and_then(Value::as_str)
                .is_some_and(|a| !a.is_empty())
        {
            let _ = st.cmd_tx.send(v);
            st.ctx.request_repaint();
        }
    }
    forward.abort();
}

/// Plain text without the `[ ]` markers.
pub fn bracket_plain(s: &str) -> String {
    s.chars().filter(|c| *c != '[' && *c != ']').collect()
}

/// HTML with `[bracketed]` parts in italics (`span.bracket-text`) and line breaks as `<br>`.
pub fn bracket_html(s: &str) -> String {
    let mut out = String::new();
    let mut open = false;
    for c in s.chars() {
        match c {
            '[' if !open => {
                out.push_str("<span class=\"bracket-text\">");
                open = true;
            }
            ']' if open => {
                out.push_str("</span>");
                open = false;
            }
            '[' | ']' => {}
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("<br>"),
            c => out.push(c),
        }
    }
    if open {
        out.push_str("</span>");
    }
    out
}

fn rgba(c: Rgba, opacity: f32) -> String {
    let [r, g, b, a] = c.0;
    format!(
        "rgba({r},{g},{b},{:.3})",
        a as f32 / 255.0 * opacity.clamp(0.0, 1.0)
    )
}

fn text_css(sel: &str, t: &TextStyle, opacity: f32, line_spacing: f32) -> String {
    let mut shadows = Vec::new();
    if t.outline {
        let w = t.outline_width.max(0.0);
        let c = rgba(t.outline_color, opacity);
        for (x, y) in [
            (1.0, 0.0),
            (-1.0, 0.0),
            (0.0, 1.0),
            (0.0, -1.0),
            (1.0, 1.0),
            (-1.0, 1.0),
            (1.0, -1.0),
            (-1.0, -1.0),
        ] {
            shadows.push(format!("{}px {}px 0 {c}", x * w, y * w));
        }
    }
    if t.shadow {
        shadows.push(format!(
            "{}px {}px {}px {}",
            t.shadow_offset[0],
            t.shadow_offset[1],
            t.shadow_blur,
            rgba(t.shadow_color, opacity)
        ));
    }
    let align = match t.align {
        HAlign::Left => "left",
        HAlign::Center => "center",
        HAlign::Right => "right",
    };
    let self_align = match t.align {
        HAlign::Left => "flex-start",
        HAlign::Center => "center",
        HAlign::Right => "flex-end",
    };
    let bg = if t.highlight {
        format!(
            "background:{};padding:0.15em 0.4em;border-radius:{}px;",
            rgba(t.highlight_color, opacity),
            t.highlight_radius
        )
    } else {
        String::new()
    };
    format!(
        "{sel}{{font-family:'{}',sans-serif;font-size:{}px;font-weight:{};color:{};text-shadow:{};text-transform:{};text-align:{align};align-self:{self_align};line-height:{};font-style:{};text-decoration:{};{bg}white-space:pre-wrap;}}\n",
        t.family.replace('\'', ""),
        t.size,
        if t.bold { "bold" } else { "normal" },
        rgba(t.color, opacity),
        if shadows.is_empty() {
            "none".to_string()
        } else {
            shadows.join(",")
        },
        if t.uppercase { "uppercase" } else { "none" },
        1.2 * line_spacing.max(0.5),
        if t.italic { "italic" } else { "normal" },
        if t.underline { "underline" } else { "none" },
    )
}

/// The overlay page's stylesheet, from the projection text settings.
/// The overlay page's stylesheet. As in the original, OBS text is always bottom-anchored
/// (50px from the bottom), whatever the projection's vertical position.
pub fn css(cfg: &OverlayConfig, portrait: bool) -> String {
    let mut s = format!(
        "#wrap{{justify-content:flex-end;padding:0 {0}px 50px {0}px;}}\n",
        cfg.padding
    );
    if cfg.border {
        s += &format!(
            "#box{{border:{}px solid {};padding:{}px {}px;}}\n",
            cfg.border_thickness,
            rgba(cfg.border_color, cfg.opacity),
            cfg.border_padding[1],
            cfg.border_padding[0]
        );
    }
    s += &text_css("#text", &cfg.text, cfg.opacity, cfg.line_spacing);
    s += &text_css("#ref", &cfg.reference, cfg.opacity, 1.0);
    s += match cfg.reference_pos {
        RefPos::Above => format!(
            "#ref{{order:-1;margin-bottom:{}px;}}\n",
            cfg.reference_spacing
        ),
        RefPos::Below => format!("#ref{{order:0;margin-top:{}px;}}\n", cfg.reference_spacing),
    }
    .as_str();
    if !cfg.show_reference {
        s += "#ref{display:none !important;}\n";
    }
    s += if portrait {
        ".media.card{top:calc(95% - 32%);width:88%;height:32%;}\n"
    } else {
        ".media.card{top:calc(95% - 40%);width:40%;height:40%;}\n"
    };
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn obs_text_is_bottom_anchored() {
        let cfg = OverlayConfig {
            vertical: crate::settings::VAlign::Center,
            ..OverlayConfig::default()
        };
        assert!(css(&cfg, false).contains("justify-content:flex-end;padding:0 40px 50px 40px"));
    }

    #[test]
    fn brackets() {
        assert_eq!(bracket_plain("a [b] c"), "a b c");
        assert_eq!(
            bracket_html("a [b<] c\nd"),
            "a <span class=\"bracket-text\">b&lt;</span> c<br>d"
        );
        assert_eq!(
            bracket_html("[open"),
            "<span class=\"bracket-text\">open</span>"
        );
    }

    #[test]
    fn data_field_names() {
        let v = serde_json::to_value(Data::default()).unwrap_or_default();
        for k in [
            "reference",
            "textHtml",
            "hasMedia",
            "mediaIsVideo",
            "hasYouTube",
            "youTubeUrl",
            "notesImageTimestamp",
            "mediaFullscreen",
        ] {
            assert!(v.get(k).is_some(), "{k}");
        }
    }
}
