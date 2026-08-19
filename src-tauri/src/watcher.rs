use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::clipboard::{is_image_file, normalize_to_png, percent_decode, ClipboardBackend};
use crate::detect::detect_kind;
use crate::store::{hash_bytes, Store};

fn png_dims(png: &[u8]) -> Option<(u32, u32)> {
    // into_dimensions reads only the PNG header — cheap enough for the 500ms
    // poll loop even on big screenshots.
    image::ImageReader::new(std::io::Cursor::new(png))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok())
}

// Does the captured text reference an image file on disk? Copying a file in a
// file manager leaks its path into the text target alongside the pixels
// ("file:///home/me/Pictures/screenshot.png"). Plain un-prefixed paths are only
// honored when the OS also reports a real file payload, so a bare
// "/home/me/foo.png" pasted from an editor stays text unless it's actually a
// file copy.
fn image_path_from_text(text: &str, has_file_payload: bool) -> Option<PathBuf> {
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let path = if let Some(rest) = line.strip_prefix("file://") {
            PathBuf::from(percent_decode(rest))
        } else if has_file_payload && line.starts_with('/') {
            PathBuf::from(line)
        } else {
            continue;
        };
        if is_image_file(&path) && path.is_file() {
            return Some(path);
        }
    }
    None
}

// Reads the image bytes off disk, normalizes to PNG (no-op for PNG input), and
// stores it as an image item named after the file. Returns true when the text
// has been handled as an image (added or already known), so it isn't also
// recorded as a text item.
fn capture_path_as_image(
    app: &AppHandle,
    path: &std::path::Path,
    last_image_hash: &mut Option<String>,
) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let Ok(png) = normalize_to_png(&bytes) else {
        return false;
    };
    let hash = hash_bytes(&png);
    if last_image_hash.as_deref() == Some(hash.as_str()) {
        return true;
    }
    let Some((w, h)) = png_dims(&png) else {
        return false;
    };
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Image".to_string());
    {
        let state = app.state::<Mutex<Store>>();
        state.lock().unwrap().add_image(&hash, &png, w, h, name);
    }
    let _ = app.emit("clipboard-updated", ());
    last_image_hash.replace(hash);
    true
}

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last_text: Option<String> = None;
        let mut last_image_hash: Option<String> = None;

        loop {
            let backend = app.state::<Mutex<Box<dyn ClipboardBackend>>>();

            // --- text -------------------------------------------------------
            // Raw read WITHOUT dedupe filtering: we need to know whether the
            // clipboard holds any text at all (to gate the image probe) even
            // when the text is unchanged from the last cycle.
            let text = backend
                .lock()
                .unwrap()
                .get_text()
                .ok()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty());

            let text_present = text.is_some();

            let mut handled_as_image = false;
            if let Some(text) = text {
                if last_text.as_deref() == Some(text.as_str()) {
                    // Same text as last cycle — nothing new to record. Keep the
                    // last_text marker so the value only re-emits "clipboard-
                    // updated" when it actually changes.
                } else {
                    // A file-manager copy of an image shows up in the text
                    // target as a path. Store the real image from disk instead
                    // of the path. Only probe the clipboard's file payload when
                    // the text looks path-like, so ordinary text copies never
                    // trigger an extra clipboard read.
                    let looks_like_path = text.starts_with('/') || text.contains("file://");
                    let has_file_payload = if looks_like_path {
                        let mut b = backend.lock().unwrap();
                        b.get_image_name().ok().flatten().is_some()
                    } else {
                        false
                    };
                    if let Some(path) = image_path_from_text(&text, has_file_payload) {
                        handled_as_image =
                            capture_path_as_image(&app, &path, &mut last_image_hash);
                    }
                    if !handled_as_image {
                        let kind = detect_kind(&text).to_string();
                        {
                            let state = app.state::<Mutex<Store>>();
                            state.lock().unwrap().add_text(&text, &kind);
                        }
                        let _ = app.emit("clipboard-updated", ());
                    }
                    last_text = Some(text);
                }
            }

            // --- image ------------------------------------------------------
            // Tracked separately from text: copying a screenshot doesn't change
            // the last-seen text and vice versa, so the two must not clobber
            // each other's dedupe state. Skipped on cycles where the text was
            // already resolved as an image from disk, avoiding a duplicate.
            //
            // Gated on the text target being ABSENT: when the clipboard carries
            // text (a plain copy, an unchanged one, or a handled image path),
            // there is almost never an image underneath, and blind-probing one
            // makes the X11 backend fall through to xclip — spawning up to five
            // subprocesses per cycle while holding the same Mutex the paste
            // path needs. Text-only clipboards therefore never touch image
            // probing at all.
            if !handled_as_image {
                if text_present {
                    // Text is on the clipboard and wasn't resolved to an image.
                    // A new text copy supersedes any previous image: reset the
                    // tracker so that re-copying that same image later registers
                    // as a fresh capture (and bumps it) instead of being skipped
                    // as "unchanged". Nothing to probe this cycle.
                    last_image_hash = None;
                } else {
                    // Clipboard holds no text — a pure image copy (screenshot
                    // tool, browser/editor "copy image") or an emptied
                    // clipboard. Probe for one.
                    last_text = None;
                    let (image, source_name) = {
                        let mut b = backend.lock().unwrap();
                        let img = b.get_image().ok().flatten();
                        let name = if img.is_some() {
                            b.get_image_name().ok().flatten()
                        } else {
                            None
                        };
                        (img, name)
                    };
                    match image {
                        Some(png) => {
                            let hash = hash_bytes(&png);
                            if last_image_hash.as_deref() != Some(hash.as_str()) {
                                if let Some((w, h)) = png_dims(&png) {
                                    let name = source_name.unwrap_or_else(|| "Image".to_string());
                                    {
                                        let state = app.state::<Mutex<Store>>();
                                        state.lock().unwrap().add_image(&hash, &png, w, h, name);
                                    }
                                    let _ = app.emit("clipboard-updated", ());
                                    last_image_hash = Some(hash);
                                }
                            }
                        }
                        None => {
                            // Clipboard holds no image now (e.g. the clipboard
                            // was emptied). Reset so that copying that same image
                            // again later registers as a fresh capture (and
                            // bumps it) instead of being skipped as "unchanged".
                            last_image_hash = None;
                        }
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    });
}