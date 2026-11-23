//! Embedded web views for the YouTube and Google tabs and YouTube projection: WebView2 on
//! Windows, WKWebView on macOS (through `wry`). Other platforms show a notice instead.

/// The YouTube video id and start second of a watch, share, shorts, live or embed URL.
pub fn youtube_id(url: &str) -> Option<(String, u32)> {
    let u = url.trim();
    let rest = u.split_once("://").map(|(_, r)| r).unwrap_or(u);
    let (host_path, query) = rest.split_once('?').unwrap_or((rest, ""));
    let (host, path) = host_path.split_once('/').unwrap_or((host_path, ""));
    let host = host
        .trim_start_matches("www.")
        .trim_start_matches("m.")
        .to_lowercase();
    let param = |k: &str| {
        query.split('&').find_map(|kv| {
            kv.split_once('=')
                .filter(|(a, _)| *a == k)
                .map(|(_, v)| v.to_string())
        })
    };
    let id = if host == "youtu.be" {
        path.split('/').next().map(str::to_string)
    } else if host.ends_with("youtube.com") || host.ends_with("youtube-nocookie.com") {
        if path.starts_with("watch") {
            param("v")
        } else {
            let mut parts = path.split('/');
            match parts.next() {
                Some("shorts" | "embed" | "live" | "v") => parts.next().map(str::to_string),
                _ => None,
            }
        }
    } else {
        None
    }?;
    let id: String = id
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if id.len() < 6 {
        return None;
    }
    let start = param("t")
        .or_else(|| param("start"))
        .map(|t| parse_time(&t))
        .unwrap_or(0);
    Some((id, start))
}

fn parse_time(t: &str) -> u32 {
    if let Ok(s) = t.parse::<u32>() {
        return s;
    }
    let (mut total, mut num) = (0u32, 0u32);
    for c in t.chars() {
        if let Some(d) = c.to_digit(10) {
            num = num.saturating_mul(10).saturating_add(d);
        } else {
            let k = match c {
                'h' => 3600,
                'm' => 60,
                _ => 1,
            };
            total = total.saturating_add(num.saturating_mul(k));
            num = 0;
        }
    }
    total.saturating_add(num)
}

/// The URL that plays a video full-window.
pub fn embed_url(url: &str, muted: bool) -> String {
    match youtube_id(url) {
        Some((id, start)) => format!(
            "https://www.youtube.com/embed/{id}?enablejsapi=1&autoplay=1&rel=0&modestbranding=1&playsinline=1&start={start}{}",
            if muted { "&mute=1" } else { "" }
        ),
        None => url.to_string(),
    }
}

/// Runs in the YouTube tab's main frame before the page's own scripts: keeps every video
/// muted (the tab's default) from the moment it appears, instead of after the next poll.
/// The Mute button flips `__spMuted` and the `__spMuted` localStorage flag, which survives
/// navigation to the next video.
pub const YT_MUTE_GUARD: &str = r#"(function(){
  var k='__spMuted'; var m=true;
  try{ m = localStorage.getItem(k) !== '0'; }catch(e){}
  window.__spMuted = m;
  function g(){
    var vs = document.querySelectorAll('video');
    for (var i=0;i<vs.length;i++){ if (vs[i].muted !== window.__spMuted) { vs[i].muted = window.__spMuted; } }
  }
  setInterval(g, 50);
  if (window.MutationObserver) {
    try { new MutationObserver(g).observe(document.documentElement || document, {childList:true, subtree:true}); } catch(e) {}
  }
})();"#;

/// Script for the Mute button: set the guard flag and apply it to the videos playing now.
pub fn yt_mute_script(muted: bool) -> String {
    format!(
        "(function(){{window.__spMuted={muted};try{{localStorage.setItem('__spMuted','{flag}')}}catch(e){{}}var vs=document.querySelectorAll('video');for(var i=0;i<vs.length;i++){{vs[i].muted={muted};}}}})()",
        muted = muted,
        flag = if muted { "1" } else { "0" },
    )
}

pub const SUPPORTED: bool = cfg!(any(windows, target_os = "macos"));

/// A localhost-only HTTP server that serves the YouTube player page. YouTube requires a
/// Referer header on embed requests, and documents loaded from about:blank (wry's
/// load_html) send none, which shows Error 153. An http://localhost origin sends one,
/// like the original app's http://localhost/ base URL — and YouTube refuses embeds whose
/// page origin is http://127.0.0.1 with "This video is unavailable", so the URL must say
/// localhost even though the socket binds 127.0.0.1 (verified on Windows/WebView2).
pub struct PlayerHttp {
    port: u16,
    _rt: tokio::runtime::Runtime,
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

async fn player_page(
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> axum::response::Html<String> {
    let src = q.get("src").map(String::as_str).unwrap_or_default();
    let src = if src.starts_with("https://") || src.starts_with("http://") {
        src
    } else {
        ""
    };
    axum::response::Html(format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>html,body,iframe{{margin:0;padding:0;width:100%;height:100%;border:0;background:#000;display:block}}</style></head><body><iframe src=\"{}\" allow=\"autoplay; encrypted-media; fullscreen\" allowfullscreen></iframe></body></html>",
        html_escape(src)
    ))
}

impl PlayerHttp {
    pub fn start() -> Result<PlayerHttp, String> {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let listener = rt.block_on(async {
            tokio::net::TcpListener::bind(("127.0.0.1", 0))
                .await
                .map_err(|e| e.to_string())
        })?;
        let port = listener.local_addr().map_err(|e| e.to_string())?.port();
        let app = axum::Router::new().route("/player", axum::routing::get(player_page));
        rt.spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                log::warn!("player server: {e}");
            }
        });
        Ok(PlayerHttp { port, _rt: rt })
    }

    /// The URL of the player page for a video, muted or not.
    pub fn url(&self, video_url: &str, muted: bool) -> String {
        let embed = embed_url(video_url, muted);
        format!(
            "http://localhost:{}/player?src={}",
            self.port,
            url_encode(&embed)
        )
    }
}

#[cfg(any(windows, target_os = "macos"))]
mod imp {
    use wry::dpi::{PhysicalPosition, PhysicalSize};
    use wry::{Rect, WebView, WebViewBuilder};

    #[derive(Default)]
    pub struct WebPane {
        view: Option<WebView>,
        loaded: String,
        visible: bool,
        init_script: Option<String>,
        pub error: Option<String>,
    }

    fn bounds(rect: egui::Rect, ppp: f32) -> Rect {
        Rect {
            position: PhysicalPosition::new(
                (rect.min.x * ppp).round() as i32,
                (rect.min.y * ppp).round() as i32,
            )
            .into(),
            size: PhysicalSize::new(
                (rect.width() * ppp).round().max(1.0) as u32,
                (rect.height() * ppp).round().max(1.0) as u32,
            )
            .into(),
        }
    }

    impl WebPane {
        /// Show the pane over `rect` (egui points) of the main window, creating it on first use.
        pub fn show(&mut self, frame: &eframe::Frame, rect: egui::Rect, ppp: f32, url: &str) {
            match &self.view {
                Some(v) => {
                    let _ = v.set_bounds(bounds(rect, ppp));
                    if !self.visible {
                        let _ = v.set_visible(true);
                        self.visible = true;
                    }
                }
                None => {
                    if self.error.is_some() {
                        return;
                    }
                    let mut b = WebViewBuilder::new()
                        .with_bounds(bounds(rect, ppp))
                        .with_url(url)
                        .with_autoplay(true)
                        .with_focused(false);
                    if let Some(js) = &self.init_script {
                        b = b.with_initialization_script(js.clone());
                    }
                    match b.build_as_child(frame) {
                        Ok(v) => {
                            self.view = Some(v);
                            self.loaded = url.to_string();
                            self.visible = true;
                        }
                        Err(e) => self.error = Some(format!("Web view unavailable: {e}")),
                    }
                }
            }
        }

        pub fn hide(&mut self) {
            if let Some(v) = &self.view
                && self.visible
            {
                let _ = v.set_visible(false);
                let _ = v.focus_parent();
                self.visible = false;
            }
        }

        /// Give keyboard focus back to the main window, so egui text fields can be typed in.
        pub fn release_focus(&self) {
            if let Some(v) = &self.view {
                let _ = v.focus_parent();
            }
        }

        /// Navigate unless already showing `url`.
        pub fn load(&mut self, url: &str) {
            if let Some(v) = &self.view
                && self.loaded != url
            {
                let _ = v.load_url(url);
                self.loaded = url.to_string();
            }
            if self.view.is_none() {
                self.loaded.clear();
            }
        }

        pub fn url(&self) -> Option<String> {
            self.view.as_ref().and_then(|v| v.url().ok())
        }

        /// Script run in the main frame of every page the pane loads, before the page's own
        /// scripts (used by the YouTube tab to keep videos muted from the first frame).
        pub fn set_init_script(&mut self, js: &str) {
            self.init_script = Some(js.to_string());
        }

        pub fn script(&self, js: &str) {
            if let Some(v) = &self.view {
                let _ = v.evaluate_script(js);
            }
        }

        pub fn close(&mut self) {
            self.view = None;
            self.loaded.clear();
            self.visible = false;
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    #[derive(Default)]
    pub struct WebPane {
        pub error: Option<String>,
    }

    impl WebPane {
        pub fn show(&mut self, _frame: &eframe::Frame, _rect: egui::Rect, _ppp: f32, _url: &str) {
            self.error = Some("The built-in browser is available on Windows and macOS.".into());
        }
        pub fn hide(&mut self) {}
        pub fn release_focus(&self) {}
        pub fn load(&mut self, _url: &str) {}
        pub fn url(&self) -> Option<String> {
            None
        }
        pub fn script(&self, _js: &str) {}
        pub fn set_init_script(&mut self, _js: &str) {}
        pub fn close(&mut self) {}
    }
}

pub use imp::WebPane;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert_eq!(
            youtube_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m5s"),
            Some(("dQw4w9WgXcQ".into(), 65))
        );
        assert_eq!(
            youtube_id("https://youtu.be/dQw4w9WgXcQ?t=42"),
            Some(("dQw4w9WgXcQ".into(), 42))
        );
        assert_eq!(
            youtube_id("youtube.com/shorts/abcdefghijk"),
            Some(("abcdefghijk".into(), 0))
        );
        assert_eq!(
            youtube_id("https://m.youtube.com/embed/abcdefghijk?start=9"),
            Some(("abcdefghijk".into(), 9))
        );
        assert_eq!(youtube_id("https://www.youtube.com/"), None);
        assert_eq!(youtube_id("https://example.com/watch?v=abcdefghijk"), None);
    }

    #[test]
    fn player_url_origin_is_localhost() {
        if let Ok(s) = PlayerHttp::start() {
            let u = s.url("https://youtu.be/f2oxGYpuLkw?list=RDf2oxGYpuLkw", false);
            assert!(u.starts_with("http://localhost:"));
            assert!(u.contains("/player?src="));
        }
    }
}
