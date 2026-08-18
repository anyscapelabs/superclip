pub mod wayland;
pub mod x11;

use std::io::{Cursor, Write};
use std::sync::atomic::{AtomicBool, Ordering};

// --- paste logging -----------------------------------------------------------
// A single append-only log all backends share, so debugging a paste that went
// to the wrong window / compositor doesn't depend on which clipboard backend
// ended up chosen at runtime.
#[cfg(target_os = "linux")]
static PASTE_LOG_FAILED: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "linux")]
pub fn paste_log(msg: &str) {
    use std::time::{SystemTime, UNIX_EPOCH};
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/superclip-paste.log")
    {
        Ok(mut f) => {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let _ = writeln!(f, "{ms}: {msg}");
        }
        Err(e) => {
            // Fail loudly exactly once per app run so a broken/unwritable log
            // can't silently fool a debugging session, but never crash the
            // paste feature itself.
            if !PASTE_LOG_FAILED.swap(true, Ordering::SeqCst) {
                eprintln!("superclip: warning: cannot write /tmp/superclip-paste.log: {e}");
            }
        }
    }
}

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

pub fn is_image_file(path: &std::path::Path) -> bool {
    const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp", "gif", "tif", "tiff"];
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

// file:// URIs arrive percent-encoded (%20 for space, %C3%A9 for é, …). Decode
// %XX escapes so extracted filenames match what the user actually sees.
pub fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Decodes any supported raster (PNG, JPEG, WebP, BMP, GIF) and re-encodes it
/// as PNG. Every captured image is normalized to PNG so storage, IPC, and the
/// frontend preview all share one format. Already-PNG input is passed through
/// untouched (no pointless re-encode of large screenshots).
pub fn normalize_to_png(bytes: &[u8]) -> Result<Vec<u8>, String> {
    if bytes.starts_with(PNG_MAGIC) {
        return Ok(bytes.to_vec());
    }
    let img = image::load_from_memory(bytes).map_err(|e| format!("image decode failed: {e}"))?;
    let rgba = img.to_rgba8();
    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("png encode failed: {e}"))?;
    Ok(buf.into_inner())
}

// ClipboardBackend abstracts the clipboard + paste layer so that session
// branching (X11 vs Wayland) lives in one place instead of scattered
// `if wayland` checks across watcher.rs, store.rs, and the commands.
//
// The backend is chosen ONCE at startup and stored as managed state. Every
// command and the watcher pull from the same instance rather than re-detecting
// the session type on every call.
pub trait ClipboardBackend: Send {
    fn get_text(&mut self) -> Result<String, String>;
    fn set_text(&mut self, text: &str) -> Result<(), String>;
    // Images flow through the same trait as PNG bytes. Backends that provide
    // raw pixels (arboard -> RGBA) encode to PNG here; backends that already
    // hand over PNG (wl-paste --type image/png) pass bytes straight through.
    // Ok(None) means the clipboard currently holds no image.
    fn get_image(&mut self) -> Result<Option<Vec<u8>>, String>;
    fn set_image(&mut self, png: &[u8]) -> Result<(), String>;
    // Best-effort name for the image currently on the clipboard — the source
    // file's basename when it was copied as a file. Ok(None) when the source
    // carries no name (raw pixel copies from screenshot tools / browsers).
    fn get_image_name(&mut self) -> Result<Option<String>, String>;
    fn send_paste(&self, target_window: Option<&str>) -> Result<(), String>;
}

pub fn detect_backend() -> Box<dyn ClipboardBackend> {
    #[cfg(target_os = "linux")]
    {
        match std::env::var("XDG_SESSION_TYPE").as_deref() {
            Ok("wayland") => Box::new(wayland::WaylandBackend::new()),
            // Missing or unrecognized XDG_SESSION_TYPE defaults to the X11
            // path — safer fallback than assuming Wayland.
            _ => Box::new(x11::X11Backend::new()),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Box::new(x11::X11Backend::new())
    }
}
