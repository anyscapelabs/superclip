use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::clipboard::ClipboardBackend;
use crate::detect::detect_kind;
use crate::store::Store;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last: Option<String> = None;

        loop {
            let backend = app.state::<Mutex<Box<dyn ClipboardBackend>>>();
            let text = backend
                .lock()
                .unwrap()
                .get_text()
                .ok()
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .filter(|t| last.as_deref() != Some(t.as_str()));

            if let Some(text) = text {
                let kind = detect_kind(&text).to_string();
                {
                    let state = app.state::<Mutex<Store>>();
                    let mut store = state.lock().unwrap();
                    store.add(text.clone(), kind);
                }
                let _ = app.emit("clipboard-updated", ());
                last = Some(text);
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    });
}
