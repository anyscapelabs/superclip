use std::io::Cursor;
use std::time::Duration;

use super::{paste_log, ClipboardBackend};

// --- X11 / default backend ---------------------------------------------------
// The original Superclip path: arboard for read/write, xdotool for synthetic
// paste. On non-Linux platforms this same struct forwards to the equivalent OS
// paste mechanism (SendKeys, osascript), since arboard + OS key-send is the
// right default everywhere.

pub struct X11Backend {
    clipboard: Option<arboard::Clipboard>,
}

impl X11Backend {
    pub fn new() -> Self {
        Self {
            // Init can fail on headless/Wayland-without-XWayland setups. Store
            // None and surface the error per-call instead of panicking at boot,
            // so the watcher and commands degrade gracefully.
            clipboard: arboard::Clipboard::new().ok(),
        }
    }
}

impl ClipboardBackend for X11Backend {
    fn get_text(&mut self) -> Result<String, String> {
        self.clipboard
            .as_mut()
            .ok_or_else(|| "clipboard unavailable (no X11)".to_string())?
            .get_text()
            .map_err(|e| e.to_string())
    }

    fn set_text(&mut self, text: &str) -> Result<(), String> {
        self.clipboard
            .as_mut()
            .ok_or_else(|| "clipboard unavailable (no X11)".to_string())?
            .set_text(text.to_string())
            .map_err(|e| e.to_string())
    }

    fn get_image(&mut self) -> Result<Option<Vec<u8>>, String> {
        // arboard only ever requests the image/png target on Linux, which
        // covers most copies. When it comes up empty — e.g. an app advertising
        // image/jpeg alone — probe the other raster targets directly via xclip
        // and normalize whatever we find to PNG.
        if let Some(cb) = self.clipboard.as_mut() {
            if let Ok(img) = cb.get_image() {
                return encode_png(img.width, img.height, &img.bytes).map(Some);
            }
        }
        #[cfg(target_os = "linux")]
        {
            Ok(xclip_image())
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(None)
        }
    }

    fn set_image(&mut self, png: &[u8]) -> Result<(), String> {
        // Fast path: hand xclip the raw PNG bytes so the receiving app gets
        // them verbatim — no decode + re-encode (~300ms on big screenshots).
        // xclip reads the temp file, claims clipboard ownership, then keeps
        // running detached serving image/png until a new selection takes over.
        // We poll TARGETS so this only returns once ownership is guaranteed
        // (measured ~100ms, comfortably under INITIAL_PASTE_DELAY), so the
        // Ctrl+V that follows never hits a stale/empty selection.
        #[cfg(target_os = "linux")]
        {
            if set_image_xclip(png) {
                return Ok(());
            }
            // xclip missing or failed to claim ownership — fall through to
            // arboard so image paste never silently breaks.
        }
        let cb = self
            .clipboard
            .as_mut()
            .ok_or_else(|| "clipboard unavailable (no X11)".to_string())?;
        let (rgba, width, height) = decode_png(png)?;
        let img = arboard::ImageData {
            width,
            height,
            bytes: rgba.into(),
        };
        cb.set_image(img).map_err(|e| e.to_string())
    }

    fn get_image_name(&mut self) -> Result<Option<String>, String> {
        let Some(cb) = self.clipboard.as_mut() else {
            return Ok(None);
        };
        // file_list reads the clipboard's file payload (text/uri-list on X11,
        // CF_HDROP on Windows, NSFilenamesPboardType on macOS). Prefer a file
        // that looks like an image, otherwise fall back to the first entry.
        match cb.get().file_list() {
            Ok(files) => Ok(files
                .iter()
                .find(|f| super::is_image_file(f))
                .or_else(|| files.first())
                .and_then(|f| f.file_name())
                .map(|n| n.to_string_lossy().into_owned())),
            Err(_) => Ok(None),
        }
    }

    fn send_paste(&self, target_window: Option<&str>) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            send_synthetic_paste(target_window);
            Ok(())
        }
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("powershell")
                .args([
                    "-NoProfile",
                    "-Command",
                    "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('^v')",
                ])
                .status();
            Ok(())
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("osascript")
                .args([
                    "-e",
                    "tell application \"System Events\" to keystroke \"v\" using command down",
                ])
                .status();
            Ok(())
        }
    }
}

// --- logging -----------------------------------------------------------------

#[cfg(target_os = "linux")]
const FOCUS_SETTLE_DELAY: Duration = Duration::from_millis(100);
#[cfg(target_os = "linux")]
const MAX_FOCUS_ATTEMPTS: u32 = 2;

#[cfg(target_os = "linux")]
pub fn active_window_id() -> Option<String> {
    // NOTE: a browser window with multiple tabs is a single X11 window — all
    // tabs share one window id. We can only target the window, not a specific
    // tab, so a paste lands in whichever tab is focused/visible at paste time.
    let out = std::process::Command::new("xdotool")
        .args(["getactivewindow"])
        .output()
        .ok()?;
    let id = String::from_utf8(out.stdout).ok()?.trim().to_string();
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

// Reads WM_CLASS via xprop. Kept (unused) for v0.2 terminal-paste support,
// which needs it to distinguish terminal apps from GUI apps. Do not delete.
#[cfg(target_os = "linux")]
#[allow(dead_code)]
pub fn window_class(id: &str) -> String {
    std::process::Command::new("xprop")
        .args(["-id", id, "WM_CLASS"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_default()
        .trim()
        .to_lowercase()
}

#[cfg(target_os = "linux")]
fn activate_window(id: &str) {
    // windowactivate --sync blocks until the WM has raised+activated the
    // window; windowfocus --sync then pins X input focus to it so synthetic
    // keys land on the right app. Neither returning Ok proves focus landed —
    // send_synthetic_paste verifies that separately.
    let act = std::process::Command::new("xdotool")
        .args(["windowactivate", "--sync", id])
        .status();
    let foc = std::process::Command::new("xdotool")
        .args(["windowfocus", "--sync", id])
        .status();
    paste_log(&format!(
        "activate id={id} windowactivate={:?} windowfocus={:?}",
        act.map(|s| s.success()),
        foc.map(|s| s.success())
    ));
    std::thread::sleep(FOCUS_SETTLE_DELAY);
}

#[cfg(target_os = "linux")]
fn focused_window_id() -> Option<String> {
    let out = std::process::Command::new("xdotool")
        .args(["getactivewindow"])
        .output()
        .ok()?;
    let id = String::from_utf8(out.stdout).ok()?.trim().to_string();
    paste_log(&format!("getactivewindow => {id:?}"));
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

#[cfg(target_os = "linux")]
fn send_synthetic_paste(target: Option<&str>) {
    // Terminal paste (xclip PRIMARY + middle-click) is deferred to v0.2; this
    // phase only handles browsers and GUI apps via a synthetic Ctrl+V.
    let Some(id) = target.map(str::trim).filter(|s| !s.is_empty()) else {
        // We never learned which window the user was in before opening the
        // picker (active_window_id() failed). Best we can do is paste into
        // whatever currently has focus; keep it explicit in the log.
        paste_log("no target window captured, falling back to ambient focus paste");
        let key = std::process::Command::new("xdotool")
            .args(["key", "ctrl+v"])
            .status();
        paste_log(&format!(
            "ambient key status={:?}",
            key.map(|s| s.success())
        ));
        return;
    };

    // A browser window with multiple tabs is a single X11 window — all tabs
    // share one window id. We can only target the window, not a specific tab,
    // so the paste lands in whichever tab is actually focused at paste time.
    let mut attempts: u32 = 0;
    loop {
        attempts += 1;
        activate_window(id);
        let focused = focused_window_id();
        if focused.as_deref() == Some(id) {
            break;
        }
        paste_log(&format!(
            "focus mismatch: focused={focused:?} want={id} attempt={attempts}"
        ));
        if attempts >= MAX_FOCUS_ATTEMPTS {
            break;
        }
    }

    // Send Ctrl+V with xdotool's XTEST path. `xdotool key --window` uses
    // XSendEvent (synthetic events) which Chromium, Electron, and most GTK
    // apps silently discard as untrusted — xdotool reports success but nothing
    // is pasted. Without `--window`, xdotool uses the XTEST extension
    // (XTestFakeKeyEvent), which is indistinguishable from a real hardware key
    // press and every app honors. Focus was already verified to be on the
    // target window above, so an XTEST key lands where the user is.
    let key = std::process::Command::new("xdotool")
        .args(["key", "ctrl+v"])
        .status();
    paste_log(&format!(
        "key ctrl+v status={:?}",
        key.map(|s| s.success())
    ));
}

// --- image encode / decode ---------------------------------------------------
// The backend stores images as PNG on disk and over IPC. arboard speaks RGBA
// pixels, so bytes are converted at the clipboard boundary: encoded on the way
// in (get_image), decoded on the way out (set_image). Wayland never touches
// these — wl-paste/wl-copy carry PNG natively.

fn encode_png(width: usize, height: usize, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::RgbaImage::from_raw(width as u32, height as u32, rgba.to_vec())
        .ok_or_else(|| format!("invalid image dimensions {width}x{height}"))?;
    let mut buf = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("png encode failed: {e}"))?;
    Ok(buf.into_inner())
}

fn decode_png(png: &[u8]) -> Result<(Vec<u8>, usize, usize), String> {
    let img = image::load_from_memory(png).map_err(|e| format!("png decode failed: {e}"))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok((rgba.into_raw(), width as usize, height as usize))
}

// xclip requests an arbitrary selection target, unlike arboard which is fixed
// to image/png. Instead of blindly downloading every raster format (up to five
// subprocesses per poll cycle), first ask the owner for its advertised target
// list — one fast call — and only fetch pixels for a target we actually see.
#[cfg(target_os = "linux")]
fn xclip_image() -> Option<Vec<u8>> {
    let targets = clipboard_targets()?;
    for mime in ["image/jpeg", "image/webp", "image/bmp", "image/gif", "image/tiff"] {
        if !targets.lines().any(|t| t.trim() == mime) {
            continue;
        }
        let out = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-t", mime, "-o"])
            .output()
            .ok()?;
        if out.status.success() && !out.stdout.is_empty() {
            if let Ok(png) = super::normalize_to_png(&out.stdout) {
                return Some(png);
            }
        }
    }
    None
}

// The clipboard owner's advertised target list ("TARGETS\nimage/png\n…"), one
// quick xclip call. Shared by get_image (which raster formats are available?)
// and set_image (did our spawned xclip claim ownership yet?).
#[cfg(target_os = "linux")]
fn clipboard_targets() -> Option<String> {
    let out = std::process::Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "TARGETS", "-o"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

// Writes the PNG to a temp file and hands it to a detached xclip serving
// image/png. Returns true only after the selection advertises image/png, so
// callers know ownership is real before they fire the paste key. The temp file
// is removed on both success (xclip buffers the image at startup, so the file
// is no longer needed) and failure.
#[cfg(target_os = "linux")]
fn set_image_xclip(png: &[u8]) -> bool {
    let tmp = std::env::temp_dir().join(format!(
        "superclip-paste-{}.png",
        std::process::id()
    ));
    if std::fs::write(&tmp, png).is_err() {
        return false;
    }
    let stdin_file = match std::fs::File::open(&tmp) {
        Ok(f) => f,
        Err(_) => {
            let _ = std::fs::remove_file(&tmp);
            return false;
        }
    };
    let spawned = std::process::Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "image/png", "-i"])
        .stdin(std::process::Stdio::from(stdin_file))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(_) => {
            let _ = std::fs::remove_file(&tmp);
            return false;
        }
    };

    // xclip reads the whole temp file at startup, but only after it has been
    // given a chance to open the fd we passed. Delete the file only once
    // ownership is confirmed — by that point the image is buffered in xclip.
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        if clipboard_targets()
            .map(|t| t.lines().any(|l| l.trim() == "image/png"))
            .unwrap_or(false)
        {
            // A single TARGETS hit can catch xclip the very instant it claims
            // selection ownership (XSetSelectionOwner) but before its
            // SelectionRequest-serving loop is fully running; a requesting app
            // could then get an empty/stale response. Requiring two consecutive
            // confirms a few ms apart bridges that fork window.
            std::thread::sleep(Duration::from_millis(20));
            if clipboard_targets()
                .map(|t| t.lines().any(|l| l.trim() == "image/png"))
                .unwrap_or(false)
            {
                let _ = std::fs::remove_file(&tmp);
                // Success: xclip keeps serving the selection until another app
                // takes the clipboard, then exits. Reap it from a background
                // thread so it never lingers as a zombie once its ownership is
                // superseded.
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return true;
            }
            // First confirm only — loop again for a fresh pair.
        }
        if std::time::Instant::now() > deadline {
            // Give up and reap the child so we don't leak a detached xclip
            // that can never serve a paste.
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&tmp);
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
