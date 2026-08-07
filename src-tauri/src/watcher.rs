use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::detect::detect_kind;
use crate::store::Store;

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last: Option<String> = None;
        let mut cb: Option<arboard::Clipboard> = None;

        loop {
            if cb.is_none() {
                cb = arboard::Clipboard::new().ok();
            }

            if let Some(clip) = cb.as_mut() {
                if let Ok(text) = clip.get_text() {
                    let text = text.trim().to_string();
                    if !text.is_empty() && last.as_deref() != Some(text.as_str()) {
                        let kind = detect_kind(&text).to_string();
                        {
                            let state = app.state::<Mutex<Store>>();
                            let mut store = state.lock().unwrap();
                            store.add(text.clone(), kind);
                        }
                        let _ = app.emit("clipboard-updated", ());
                        last = Some(text);
                    }
                }
            }

            std::thread::sleep(Duration::from_millis(500));
        }
    });
}
