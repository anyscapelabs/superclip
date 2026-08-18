use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct ImageEntry {
    // Path relative to the app data directory, e.g. "images/<id>.png".
    pub path: String,
    pub width: u32,
    pub height: u32,
    // Where the image came from, e.g. the source file's basename when copied
    // from a file manager ("screenshot.png"). Falls back to "Image" for raw
    // pixel copies (screenshot tools, browser "copy image").
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ClipItem {
    pub id: String,
    pub text: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub time: u64,
    pub pinned: bool,
    // `#[serde(default)]` keeps history.json files written by pre-image
    // versions (v0.1.1 and earlier) readable without the field.
    #[serde(default)]
    pub image: Option<ImageEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct History {
    pub version: u32,
    pub hotkey: String,
    pub items: Vec<ClipItem>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            version: 1,
            hotkey: "CommandOrControl+Shift+V".into(),
            items: vec![],
        }
    }
}

pub struct Store {
    pub path: PathBuf,
    pub history: History,
}

const MAX_ITEMS: usize = 100;

impl Store {
    pub fn new(path: PathBuf) -> Self {
        let history = Self::read(&path);
        Self { path, history }
    }

    fn read(path: &PathBuf) -> History {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(h) = serde_json::from_str(&data) {
                return h;
            }
        }
        History::default()
    }

    pub fn save(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if let Ok(data) = serde_json::to_string_pretty(&self.history) {
            let _ = fs::write(&self.path, data);
        }
    }

    pub fn add_text(&mut self, text: &str, kind: &str) -> String {
        let id = hash_bytes(text.as_bytes());
        self.upsert(ClipItem {
            id: id.clone(),
            text: text.to_string(),
            kind: kind.to_string(),
            time: now_ms(),
            pinned: false,
            image: None,
        });
        id
    }

    pub fn add_image(&mut self, id: &str, png: &[u8], width: u32, height: u32, name: String) {
        let path = format!("images/{id}.png");
        self.write_image(&path, png);
        self.upsert(ClipItem {
            id: id.to_string(),
            text: String::new(),
            kind: "image".to_string(),
            time: now_ms(),
            pinned: false,
            image: Some(ImageEntry {
                path,
                width,
                height,
                name,
            }),
        });
    }

    // Persists an image next to history.json under <app_data>/images/<id>.png.
    // Skips writing when the file already exists — re-copying or pasting a
    // known image would otherwise rewrite a multi-MB PNG on every bump.
    fn write_image(&self, rel: &str, png: &[u8]) {
        let Some(dir) = self.path.parent() else {
            return;
        };
        let full = dir.join(rel);
        if full.exists() {
            return;
        }
        if let Some(parent) = full.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(full, png);
    }

    // Loads the stored PNG bytes for an image item back off disk.
    pub fn image_bytes(&self, id: &str) -> Option<Vec<u8>> {
        let entry = self.get(id)?.image?;
        fs::read(self.path.parent()?.join(&entry.path)).ok()
    }

    fn upsert(&mut self, item: ClipItem) {
        let now = now_ms();
        if let Some(existing) = self.history.items.iter().find(|i| i.id == item.id) {
            // Re-copy bumps the item to the top and preserves its pin.
            let mut bumped = item;
            bumped.pinned = existing.pinned;
            bumped.time = now;
            self.history.items.retain(|i| i.id != bumped.id);
            self.history.items.insert(0, bumped);
        } else {
            self.history.items.insert(0, item);
        }

        self.trim();
        self.save();
    }

    fn trim(&mut self) {
        let pinned = self.history.items.iter().filter(|i| i.pinned).count();
        let mut unpinned_seen = 0;
        let max_unpinned = MAX_ITEMS.saturating_sub(pinned);
        self.history.items.retain(|i| {
            if i.pinned {
                true
            } else {
                unpinned_seen += 1;
                unpinned_seen <= max_unpinned
            }
        });
    }

    pub fn toggle_pin(&mut self, id: &str) {
        if let Some(item) = self.history.items.iter_mut().find(|i| i.id == id) {
            item.pinned = !item.pinned;
        }
        self.save();
    }

    pub fn clear_unpinned(&mut self) {
        self.history.items.retain(|i| i.pinned);
        self.save();
    }

    pub fn get(&self, id: &str) -> Option<ClipItem> {
        self.history.items.iter().find(|i| i.id == id).cloned()
    }
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
