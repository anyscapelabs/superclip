pub mod wayland;
pub mod x11;

use std::io::Write;
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
