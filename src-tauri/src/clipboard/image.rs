use std::time::Duration;

use super::ClipboardBackend;

// --- IMAGE clipboard path ----------------------------------------------------
// Dedicated writer for the image half of the clipboard. On X11 the write is a
// pure xclip subprocess: stateless, bounded, and holding no lock. On Wayland it
// falls back to wl-copy via the backend. This file owns every xclip read/poll
// helper, so the image path's subprocess behavior lives here and nowhere else.

const XCLIP_TIMEOUT: Duration = Duration::from_millis(1500);

// TARGETS probes are tiny and answered in ~5-50ms by a healthy owner, so a
// short bound is enough. Keeping it short is load-bearing: a wedged previous
// owner (arboard's server after a text paste, a Chromium/Electron holder, a
// SIGSTOPped X client, …) can swallow a probe for as long as the timeout, and
// the confirm loop retries rather than dying on one stale probe.
const TARGETS_PROBE_TIMEOUT: Duration = Duration::from_millis(250);

/// Sets the clipboard's image content for a paste/copy. Called under a dedicated
/// Mutex managed by lib.rs, never the shared clipboard backend's lock.
pub struct ImageClipboard {
    io: Box<dyn ClipboardBackend>,
}

impl ImageClipboard {
    pub fn new() -> Self {
        Self {
            io: super::detect_backend(),
        }
    }

    /// Puts a PNG on the clipboard. On X11 this spawns a detached xclip serving
    /// image/png and only returns once the selection advertises image/png
    /// (bounded). On Wayland it hands the bytes to wl-copy.
    pub fn write(&mut self, png: &[u8]) -> Result<(), String> {
        self.io.set_image(png)
    }
}

// Runs xclip as a subprocess with a hard deadline, returning its full stdout.
// Unlike Command::output(), a non-responsive clipboard owner can never block
// the caller forever: on timeout the child is killed and None is returned.
// stdout is drained on background threads so a large payload (a multi-MB image)
// can't deadlock the poll loop against a full pipe buffer.
#[cfg(target_os = "linux")]
pub(crate) fn run_xclip_bounded(
    args: &[&str],
    timeout: Duration,
) -> Option<std::process::Output> {
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
                    // Kill the child so the reader threads' read_to_end returns.
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

// Reads the clipboard's image bytes through xclip, trying the advertised raster
// targets in order and normalizing the first hit to PNG. Bounded so a
// non-responsive owner can never wedge the caller.
#[cfg(target_os = "linux")]
pub(crate) fn xclip_image() -> Option<Vec<u8>> {
    let targets = clipboard_targets()?;
    for mime in ["image/jpeg", "image/webp", "image/bmp", "image/gif", "image/tiff"] {
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

// The clipboard owner's advertised target list ("TARGETS\nimage/png\n…"), one
// quick xclip call. Shared by get_image (which raster formats are available?)
// and set_image (did our spawned xclip claim ownership yet?). Bounded so a
// non-responsive owner can never wedge the caller.
#[cfg(target_os = "linux")]
pub(crate) fn clipboard_targets() -> Option<String> {
    let out = run_xclip_bounded(&["-t", "TARGETS", "-o"], XCLIP_TIMEOUT)?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).into_owned())
}

// Ownership-confirm variant of clipboard_targets: same probe but with the short
// 250ms bound, so a single wedged stale owner costs one retry instead of eating
// the entire confirm budget.
#[cfg(target_os = "linux")]
fn clipboard_targets_short() -> Option<String> {
    let out = run_xclip_bounded(&["-t", "TARGETS", "-o"], TARGETS_PROBE_TIMEOUT)?;
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
pub(crate) fn set_image_xclip(png: &[u8]) -> bool {
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
    //
    // The confirm loop uses the SHORT probe timeout on purpose: ownership
    // transfers to our fresh xclip within ~50ms, and a healthy owner answers in
    // <50ms, so a probe either succeeds fast or stamps on a wedged previous
    // owner for at most 250ms before we retry. A single stale owner can never
    // hold the loop hostage for the full deadline.
    let deadline = std::time::Instant::now() + Duration::from_millis(1000);
    loop {
        if clipboard_targets_short()
            .map(|t| t.lines().any(|l| l.trim() == "image/png"))
            .unwrap_or(false)
        {
            // A single TARGETS hit can catch xclip the very instant it claims
            // selection ownership (XSetSelectionOwner) but before its
            // SelectionRequest-serving loop is fully running; a requesting app
            // could then get an empty/stale response. Requiring two consecutive
            // confirms a few ms apart bridges that fork window.
            std::thread::sleep(Duration::from_millis(20));
            if clipboard_targets_short()
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