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
            Ok(super::image::xclip_image())
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
            if super::image::set_image_xclip(png) {
                return Ok(());
            }
            // NO arboard fallback here: arboard's Linux set_image can block
            // indefinitely while a non-responsive selection owner holds the
            // clipboard (Chromium/Electron owners are notorious for ignoring
            // requests). That blocked the whole paste pipeline this session —
            // the stuck thread held the shared clipboard Mutex forever and
            // every later paste hung behind it. Fail fast instead: the caller
            // surfaces the error and the user can retry; a hang would never
            // recover without killing the app.
            Err(
                "xclip image claim failed (clipboard owner did not respond in time)".to_string(),
            )
        }
        #[cfg(not(target_os = "linux"))]
        {
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
const SYNC_TIMEOUT: Duration = Duration::from_millis(1500);

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
    // send_synthetic_paste verifies that separately. Both are bounded by a
    // watchdog: --sync has no native timeout and some WMs/apps never
    // acknowledge, which would otherwise hang the paste thread forever.
    let act = run_with_timeout(xdotool("windowactivate", "--sync", id));
    let foc = run_with_timeout(xdotool("windowfocus", "--sync", id));
    paste_log(&format!(
        "activate id={id} windowactivate={act:?} windowfocus={foc:?}"
    ));
    std::thread::sleep(FOCUS_SETTLE_DELAY);
}

// Runs an xclip invocation with a hard deadline, capturing stdout. `--sync`
// variants wait for a WM acknowledgment that never arrives on some setups;
// killing the child after SYNC_TIMEOUT turns a permanent hang into a bounded
// failure that the focus-verification retry loop can recover from.
#[cfg(target_os = "linux")]
fn run_with_timeout(mut cmd: std::process::Command) -> Option<bool> {
    let Ok(mut child) = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return None;
    };
    let deadline = std::time::Instant::now() + SYNC_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status.success()),
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Some(false);
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(target_os = "linux")]
fn xdotool(op: &str, flag: &str, id: &str) -> std::process::Command {
    let mut c = std::process::Command::new("xdotool");
    c.args([op, flag, id]);
    c
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
        let key = run_with_timeout({
            let mut c = std::process::Command::new("xdotool");
            c.args(["key", "ctrl+v"]);
            c
        });
        paste_log(&format!(
            "ambient key status={key:?}"
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
    let key = run_with_timeout({
        let mut c = std::process::Command::new("xdotool");
        c.args(["key", "ctrl+v"]);
        c
    });
    paste_log(&format!("key ctrl+v status={key:?}"));
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

#[cfg(not(target_os = "linux"))]
fn decode_png(png: &[u8]) -> Result<(Vec<u8>, usize, usize), String> {
    let img = image::load_from_memory(png).map_err(|e| format!("png decode failed: {e}"))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    Ok((rgba.into_raw(), width as usize, height as usize))
}
