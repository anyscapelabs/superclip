use std::io::Write;
use std::process::{Command, Stdio};

use super::{paste_log, ClipboardBackend};

const SUPPORTED_IMAGE_TYPES: &[&str] = &[
    "image/png",
    "image/jpeg",
    "image/webp",
    "image/bmp",
    "image/gif",
];

pub struct WaylandBackend;

impl WaylandBackend {
    pub fn new() -> Self {
        Self
    }

    fn first_supported_type(&self) -> Result<Option<String>, String> {
        let out = Command::new("wl-paste")
            .arg("--list-types")
            .output()
            .map_err(|e| format!("wl-paste --list-types failed: {e}"))?;
        if !out.status.success() {
            return Ok(None);
        }
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let t = line.trim();
            if SUPPORTED_IMAGE_TYPES.contains(&t) {
                return Ok(Some(t.to_string()));
            }
        }
        Ok(None)
    }
}

impl ClipboardBackend for WaylandBackend {
    fn get_text(&mut self) -> Result<String, String> {
        let out = Command::new("wl-paste")
            .output()
            .map_err(|e| format!("wl-paste failed: {e}"))?;
        String::from_utf8(out.stdout).map_err(|e| format!("wl-paste produced invalid UTF-8: {e}"))
    }

    fn set_text(&mut self, text: &str) -> Result<(), String> {
        let mut child = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("wl-copy failed to start: {e}"))?;
        child
            .stdin
            .take()
            .ok_or_else(|| "wl-copy stdin unavailable".to_string())?
            .write_all(text.as_bytes())
            .map_err(|e| format!("write to wl-copy failed: {e}"))?;
        Ok(())
    }

    fn get_image(&mut self) -> Result<Option<Vec<u8>>, String> {
        let Some(mime) = self.first_supported_type()? else {
            return Ok(None);
        };
        let out = Command::new("wl-paste")
            .arg("--type")
            .arg(&mime)
            .output()
            .map_err(|e| format!("wl-paste failed: {e}"))?;
        if !out.status.success() || out.stdout.is_empty() {
            return Ok(None);
        }
        super::normalize_to_png(&out.stdout).map(Some)
    }

    fn set_image(&mut self, png: &[u8]) -> Result<(), String> {
        let mut child = Command::new("wl-copy")
            .arg("--type")
            .arg("image/png")
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("wl-copy failed to start: {e}"))?;
        child
            .stdin
            .take()
            .ok_or_else(|| "wl-copy stdin unavailable".to_string())?
            .write_all(png)
            .map_err(|e| format!("write to wl-copy failed: {e}"))?;
        Ok(())
    }

    fn get_image_name(&mut self) -> Result<Option<String>, String> {
        let out = Command::new("wl-paste")
            .arg("--type")
            .arg("text/uri-list")
            .output()
            .map_err(|e| format!("wl-paste failed: {e}"))?;
        if !out.status.success() || out.stdout.is_empty() {
            return Ok(None);
        }
        let mut best: Option<String> = None;
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let line = line.trim();
            let Some(rest) = line.strip_prefix("file://") else {
                continue;
            };
            let path = super::percent_decode(rest);
            let p = std::path::Path::new(&path);
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(path.clone());
            if best.is_none() {
                best = Some(name.clone());
            }
            if super::is_image_file(p) || super::is_image_file(std::path::Path::new(&name)) {
                return Ok(Some(name));
            }
        }
        Ok(best)
    }

    fn send_paste(&self, _target_window: Option<&str>) -> Result<(), String> {
        if ydotool_available() {
            let status = Command::new("ydotool")
                .args(["key", "29:1", "47:1", "47:0", "29:0"])
                .status()
                .map_err(|e| format!("ydotool failed to start: {e}"))?;
            if status.success() {
                Ok(())
            } else {
                paste_log("ydotool key ctrl+v returned non-zero; paste may not have landed");
                Ok(())
            }
        } else {
            paste_log(
                "ydotool not available — copied to clipboard, please press Ctrl+V in the target app",
            );
            Ok(())
        }
    }
}

fn ydotool_available() -> bool {
    Command::new("ydotool").arg("--version").output().is_ok()
}
