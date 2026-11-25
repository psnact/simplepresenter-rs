# SimplePresenter

Bible and lyrics projection for churches. Runs on Windows x64 and macOS (Apple Silicon and Intel, universal build).

## Build and run

```
cargo run --release
```

Bundled resources (fonts, sample Bible, sample songs, logo) live in `resources/`. They are found next to the executable, in `../Resources/resources` inside a macOS app bundle, or via `SIMPLEPRESENTER_RESOURCES`.

User data (Bibles, songs, services, media, settings.json) lives in the per-user data folder (`%APPDATA%\psnact\SimplePresenter\data` on Windows, `~/Library/Application Support/com.psnact.SimplePresenter` on macOS). On Windows, `C:\SimplePresenter\Bible` is still used when it has Bibles and the new folder does not.

## Status

| Area | State |
|---|---|
| Bible XML (Zefania + legacy), reference parsing, search | Done |
| Songs (.xml and .txt), sections, search | Done |
| Playlists (`.service` JSON, same format as the Qt app) | Done |
| Live preview, projector window (F11 fullscreen), text styling, image backgrounds | Done |
| Video and audio playback (H.264, HEVC, VP9, AV1; MP3, AAC, WAV, FLAC, Ogg, Opus), video backgrounds | Done |
| Overlay server: OBS `/overlay` (`?portrait`), `/data`, phone `/remote`, `/projection.jpg`, `/projection.mjpg`, WebSocket on port+1 | Done |
| PowerPoint (.pptx imported and rendered in-process by DeckCraft; library, slide grid, Start/Prev/Next/End, playlist, remote) | Done |
| YouTube and Google tabs (WebView2 / WKWebView via wry), YouTube projection in a borderless player window | Done (Windows, macOS) |
| Song editor ([Chorus]-style markers), medley builder, delete songs | Done |
| Settings: Bible/Song style, OBS Overlay URLs, general (theme, interface size, output size, port) | Done |
| Update check (same `update.json` format; Help > Check for Updates) | Done |

## Shortcuts

Ctrl+N/O/S service, Ctrl+B Bible reference, Ctrl+L song search, Space go live, arrows/PageUp/PageDown previous/next, Esc clear, B black screen, F11 projector fullscreen.

## Overlay server

On by default on port 8080 (Tools menu shows the URLs; port in Settings). Add
`http://<this-computer>:8080/overlay` as an OBS browser source (1920x1080, `?portrait` for vertical).
Open `http://<this-computer>:8080/remote` on a phone on the same network. The remote's WebSocket
uses port 8081, so allow both ports through the firewall.
