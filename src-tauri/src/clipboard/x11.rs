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

    // Send Ctrl+V directly to the target window (XSendEvent, so it does not
    // depend on a keyboard grab / XTEST). Focus was verified above, so this is
    // belt-and-suspenders on top of a focused-window send.
    let key = std::process::Command::new("xdotool")
        .args(["key", "--window", id, "ctrl+v"])
        .status();
    paste_log(&format!(
        "key --window {id} ctrl+v status={:?}",
        key.map(|s| s.success())
    ));
}
