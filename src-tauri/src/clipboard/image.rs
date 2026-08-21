use std::time::Duration;

use super::ClipboardBackend;

const XCLIP_TIMEOUT: Duration = Duration::from_millis(1500);
const TARGETS_PROBE_TIMEOUT: Duration = Duration::from_millis(250);

pub struct ImageClipboard {
    io: Box<dyn ClipboardBackend>,
}

impl ImageClipboard {
    pub fn new() -> Self {
        Self {
            io: super::detect_backend(),
        }
    }

    pub fn write(&mut self, png: &[u8]) -> Result<(), String> {
        self.io.set_image(png)
    }
}

// Drain both pipes while waiting so xclip cannot block on a full buffer.
#[cfg(target_os = "linux")]
pub(crate) fn run_xclip_bounded(args: &[&str], timeout: Duration) -> Option<std::process::Output> {
    use std::io::Read as _;
    let mut child = std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .ok()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let read_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stdout {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let read_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stderr {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let deadline = std::time::Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if std::time::Instant::now() > deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => return None,
        }
    };
    Some(std::process::Output {
        status,
        stdout: read_out.join().unwrap_or_default(),
        stderr: read_err.join().unwrap_or_default(),
    })
}

#[cfg(target_os = "linux")]
pub(crate) fn xclip_image() -> Option<Vec<u8>> {
    let targets = clipboard_targets()?;
    for mime in [
        "image/jpeg",
        "image/webp",
        "image/bmp",
        "image/gif",
        "image/tiff",
    ] {
        if !targets.lines().any(|t| t.trim() == mime) {
            continue;
        }
        let out = run_xclip_bounded(&["-t", mime, "-o"], XCLIP_TIMEOUT)?;
        if out.status.success() && !out.stdout.is_empty() {
            if let Ok(png) = super::normalize_to_png(&out.stdout) {
                return Some(png);
            }
        }
    }
    None
}

#[cfg(target_os = "linux")]
pub(crate) fn clipboard_targets() -> Option<String> {
    let out = run_xclip_bounded(&["-t", "TARGETS", "-o"], XCLIP_TIMEOUT)?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(target_os = "linux")]
fn clipboard_targets_short() -> Option<String> {
    let out = run_xclip_bounded(&["-t", "TARGETS", "-o"], TARGETS_PROBE_TIMEOUT)?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(target_os = "linux")]
pub(crate) fn set_image_xclip(png: &[u8]) -> bool {
    let tmp = std::env::temp_dir().join(format!("superclip-paste-{}.png", std::process::id()));
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

    let deadline = std::time::Instant::now() + Duration::from_millis(1000);
    loop {
        if clipboard_targets_short()
            .map(|t| t.lines().any(|l| l.trim() == "image/png"))
            .unwrap_or(false)
        {
            std::thread::sleep(Duration::from_millis(20));
            if clipboard_targets_short()
                .map(|t| t.lines().any(|l| l.trim() == "image/png"))
                .unwrap_or(false)
            {
                let _ = std::fs::remove_file(&tmp);
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return true;
            }
        }
        if std::time::Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&tmp);
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}
