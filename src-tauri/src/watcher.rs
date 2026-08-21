use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::clipboard::{is_image_file, normalize_to_png, percent_decode, ClipboardBackend};
use crate::detect::detect_kind;
use crate::store::{hash_bytes, Store};

const POLL_INTERVAL: Duration = Duration::from_millis(500);

pub fn start(app: AppHandle) {
    std::thread::spawn(move || ClipboardWatcher::new(app).run());
}

struct ClipboardWatcher {
    app: AppHandle,
    backend: Box<dyn ClipboardBackend>,
    last_text: Option<String>,
    last_image_hash: Option<String>,
}

impl ClipboardWatcher {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            // A stalled clipboard read affects capture only, never paste.
            backend: crate::clipboard::detect_backend(),
            last_text: None,
            last_image_hash: None,
        }
    }

    fn run(mut self) {
        loop {
            self.capture();
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    fn capture(&mut self) {
        let text = self.read_text();
        let text_present = text.is_some();
        let captured_path_image = self.capture_text(text);
        self.capture_image(text_present, captured_path_image);
    }

    fn read_text(&mut self) -> Option<String> {
        self.backend
            .get_text()
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    }

    fn capture_text(&mut self, text: Option<String>) -> bool {
        let Some(text) = text else {
            return false;
        };
        if self.last_text.as_deref() == Some(text.as_str()) {
            return false;
        }

        let captured_path_image = self.capture_path_image(&text);
        if !captured_path_image {
            let kind = detect_kind(&text).to_string();
            self.app
                .state::<Mutex<Store>>()
                .lock()
                .unwrap()
                .add_text(&text, &kind);
            self.emit_update();
        }
        self.last_text = Some(text);
        captured_path_image
    }

    fn capture_path_image(&mut self, text: &str) -> bool {
        let looks_like_path = text.starts_with('/') || text.contains("file://");
        let has_file_payload =
            looks_like_path && self.backend.get_image_name().ok().flatten().is_some();
        let Some(path) = image_path_from_text(text, has_file_payload) else {
            return false;
        };
        self.store_file_image(&path)
    }

    fn capture_image(&mut self, text_present: bool, captured_path_image: bool) {
        if captured_path_image {
            return;
        }
        if text_present {
            self.last_image_hash = None;
            return;
        }

        self.last_text = None;
        let image = self.backend.get_image().ok().flatten();
        let name = image
            .as_ref()
            .and_then(|_| self.backend.get_image_name().ok().flatten());
        let Some(png) = image else {
            self.last_image_hash = None;
            return;
        };

        let hash = hash_bytes(&png);
        if self.last_image_hash.as_deref() == Some(hash.as_str()) {
            return;
        }
        let Some((width, height)) = png_dimensions(&png) else {
            return;
        };

        self.store_image(
            hash,
            png,
            width,
            height,
            name.unwrap_or_else(|| "Image".into()),
        );
    }

    fn store_file_image(&mut self, path: &Path) -> bool {
        let Ok(bytes) = std::fs::read(path) else {
            return false;
        };
        let Ok(png) = normalize_to_png(&bytes) else {
            return false;
        };
        let hash = hash_bytes(&png);
        if self.last_image_hash.as_deref() == Some(hash.as_str()) {
            return true;
        }
        let Some((width, height)) = png_dimensions(&png) else {
            return false;
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Image".into());

        self.store_image(hash, png, width, height, name);
        true
    }

    fn store_image(&mut self, hash: String, png: Vec<u8>, width: u32, height: u32, name: String) {
        self.app
            .state::<Mutex<Store>>()
            .lock()
            .unwrap()
            .add_image(&hash, &png, width, height, name);
        self.last_image_hash = Some(hash);
        self.emit_update();
    }

    fn emit_update(&self) {
        let _ = self.app.emit("clipboard-updated", ());
    }
}

fn png_dimensions(png: &[u8]) -> Option<(u32, u32)> {
    image::ImageReader::new(std::io::Cursor::new(png))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
}

fn image_path_from_text(text: &str, has_file_payload: bool) -> Option<PathBuf> {
    text.lines().map(str::trim).find_map(|line| {
        let path = match line.strip_prefix("file://") {
            Some(path) => PathBuf::from(percent_decode(path)),
            None if has_file_payload && line.starts_with('/') => PathBuf::from(line),
            None => return None,
        };
        (is_image_file(&path) && path.is_file()).then_some(path)
    })
}
