use std::io::{Cursor, Read};
use std::time::Duration;

use super::{paste_log, ClipboardBackend};

pub struct X11Backend {
    clipboard: Option<arboard::Clipboard>,
}

impl X11Backend {
    pub fn new() -> Self {
        Self {
            // Headless and XWayland-free sessions fail per operation, not at startup.
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
        // xclip covers raster formats arboard does not request on Linux.
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
        // xclip serves the original PNG without a decode/re-encode cycle.
        #[cfg(target_os = "linux")]
        {
            if super::image::set_image_xclip(png) {
                return Ok(());
            }
            // arboard image writes can block behind an unresponsive X11 owner.
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
        // Prefer an image path, then preserve the first available filename.
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

#[cfg(target_os = "linux")]
const FOCUS_SETTLE_DELAY: Duration = Duration::from_millis(100);
#[cfg(target_os = "linux")]
const MAX_FOCUS_ATTEMPTS: u32 = 2;
#[cfg(target_os = "linux")]
const SYNC_TIMEOUT: Duration = Duration::from_millis(1500);

#[cfg(target_os = "linux")]
pub fn active_window_id() -> Option<String> {
    let id = xdotool_window_id()?;
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

#[cfg(target_os = "linux")]
fn activate_window(id: &str) {
    // --sync waits on the window manager; both calls have a deadline.
    let act = run_with_timeout(xdotool("windowactivate", "--sync", id));
    let foc = run_with_timeout(xdotool("windowfocus", "--sync", id));
    paste_log(&format!(
        "activate id={id} windowactivate={act:?} windowfocus={foc:?}"
    ));
    std::thread::sleep(FOCUS_SETTLE_DELAY);
}

// Bounds xdotool operations that can wait indefinitely for a window manager.
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
    let id = xdotool_window_id()?;
    paste_log(&format!("getactivewindow => {id:?}"));
    if id.is_empty() {
        None
    } else {
        Some(id)
    }
}

// Reading the active window runs on the hotkey path, so it is also bounded.
#[cfg(target_os = "linux")]
fn xdotool_window_id() -> Option<String> {
    let mut child = std::process::Command::new("xdotool")
        .arg("getactivewindow")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let deadline = std::time::Instant::now() + SYNC_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            Ok(Some(_)) | Err(_) => return None,
            Ok(None) if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    let mut output = String::new();
    stdout.read_to_string(&mut output).ok()?;
    let id = output.trim().to_string();
    (!id.is_empty()).then_some(id)
}

#[cfg(target_os = "linux")]
fn send_synthetic_paste(target: Option<&str>) {
    let Some(id) = target.map(str::trim).filter(|s| !s.is_empty()) else {
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

    // Omitting --window selects xdotool's XTEST path.
    let key = run_with_timeout({
        let mut c = std::process::Command::new("xdotool");
        c.args(["key", "ctrl+v"]);
        c
    });
    paste_log(&format!("key ctrl+v status={key:?}"));
}

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
