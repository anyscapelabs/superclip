use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct ClipItem {
    pub id: String,
    pub text: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub time: u64,
    pub pinned: bool,
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

    pub fn add(&mut self, text: String, kind: String) {
        let id = hash_id(&text);
        let now = now_ms();

        if let Some(existing) = self.history.items.iter().find(|i| i.id == id) {
            let mut bumped = existing.clone();
            bumped.time = now;
            self.history.items.retain(|i| i.id != id);
            self.history.items.insert(0, bumped);
        } else {
            self.history.items.insert(
                0,
                ClipItem {
                    id,
                    text,
                    kind,
                    time: now,
                    pinned: false,
                },
            );
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

fn hash_id(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let bytes = hasher.finalize();
    bytes.iter().take(8).map(|b| format!("{:02x}", b)).collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
