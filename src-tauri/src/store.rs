use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    Beta,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ImageEntry {
    pub path: String,
    pub width: u32,
    pub height: u32,
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
    #[serde(default)]
    pub image: Option<ImageEntry>,
}

#[derive(Serialize, Deserialize)]
pub struct History {
    pub version: u32,
    pub hotkey: String,
    pub items: Vec<ClipItem>,
    #[serde(default)]
    pub channel: UpdateChannel,
}

impl Default for History {
    fn default() -> Self {
        Self {
            version: 1,
            hotkey: "CommandOrControl+Shift+V".into(),
            items: vec![],
            channel: UpdateChannel::default(),
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

    fn read(path: &Path) -> History {
        if let Ok(data) = fs::read_to_string(path) {
            if let Ok(h) = serde_json::from_str(&data) {
                return h;
            }
        }
        History::default()
    }

    // Persist before replacing the current history file.
    fn save(&self) -> bool {
        let Some(dir) = self.path.parent() else {
            return false;
        };
        if fs::create_dir_all(dir).is_err() {
            return false;
        }
        let Ok(data) = serde_json::to_string_pretty(&self.history) else {
            return false;
        };

        let tmp = self.path.with_extension("json.tmp");
        let result = (|| -> std::io::Result<()> {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(data.as_bytes())?;
            file.sync_all()?;
            replace_file(&tmp, &self.path)?;

            #[cfg(unix)]
            {
                let _ = fs::File::open(dir).and_then(|file| file.sync_all());
            }
            Ok(())
        })();

        if let Err(error) = result {
            let _ = fs::remove_file(&tmp);
            eprintln!("superclip: could not save clipboard history: {error}");
            return false;
        }
        true
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

    pub fn image_bytes(&self, id: &str) -> Option<Vec<u8>> {
        let entry = self.get(id)?.image?;
        fs::read(self.path.parent()?.join(&entry.path)).ok()
    }

    fn upsert(&mut self, item: ClipItem) {
        let now = now_ms();
        if let Some(existing) = self.history.items.iter().find(|i| i.id == item.id) {
            let mut bumped = item;
            bumped.pinned = existing.pinned;
            bumped.time = now;
            self.history.items.retain(|i| i.id != bumped.id);
            self.history.items.insert(0, bumped);
        } else {
            self.history.items.insert(0, item);
        }

        self.trim();
        if self.save() {
            self.cleanup_orphaned_images();
        }
    }

    pub fn bump(&mut self, id: &str) {
        if self.history.items.first().is_some_and(|i| i.id == id) {
            return;
        }
        if let Some(pos) = self.history.items.iter().position(|i| i.id == id) {
            let mut item = self.history.items.remove(pos);
            item.time = now_ms();
            self.history.items.insert(0, item);
            self.save();
        }
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
        if self.save() {
            self.cleanup_orphaned_images();
        }
    }

    pub fn get(&self, id: &str) -> Option<ClipItem> {
        self.history.items.iter().find(|i| i.id == id).cloned()
    }

    pub fn channel(&self) -> UpdateChannel {
        self.history.channel
    }

    pub fn set_channel(&mut self, channel: UpdateChannel) {
        self.history.channel = channel;
        self.save();
    }

    fn cleanup_orphaned_images(&self) {
        let Some(root) = self.path.parent() else {
            return;
        };
        let images = root.join("images");
        let Ok(entries) = fs::read_dir(&images) else {
            return;
        };
        let referenced: std::collections::HashSet<&str> = self
            .history
            .items
            .iter()
            .filter_map(|item| item.image.as_ref().map(|image| image.path.as_str()))
            .collect();

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let relative = format!("images/{name}");
            if !referenced.contains(relative.as_str()) {
                let _ = fs::remove_file(path);
            }
        }
    }
}

#[cfg(not(windows))]
fn replace_file(tmp: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(tmp, destination)
}

#[cfg(windows)]
fn replace_file(tmp: &Path, destination: &Path) -> std::io::Result<()> {
    let backup = destination.with_extension("json.bak");
    let _ = fs::remove_file(&backup);
    let had_destination = destination.exists();
    if had_destination {
        fs::rename(destination, &backup)?;
    }
    if let Err(error) = fs::rename(tmp, destination) {
        if had_destination {
            let _ = fs::rename(&backup, destination);
        }
        return Err(error);
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

pub fn hash_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest
        .iter()
        .take(8)
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_path(name: &str) -> PathBuf {
        let unique = format!("superclip-store-test-{name}-{}", now_ms());
        std::env::temp_dir().join(unique).join("history.json")
    }

    #[test]
    fn save_writes_valid_json_without_a_partial_file() {
        let path = test_path("atomic");
        let mut store = Store::new(path.clone());
        store.add_text("durable clipboard entry", "text");

        let saved = fs::read_to_string(&path).unwrap();
        let history: History = serde_json::from_str(&saved).unwrap();
        assert_eq!(history.items.len(), 1);
        assert!(!path.with_extension("json.tmp").exists());

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn clearing_history_releases_image_payloads() {
        let path = test_path("cleanup");
        let mut store = Store::new(path.clone());
        let id = "test-image";
        store.add_image(id, b"not-a-real-png", 1, 1, "Image".into());
        let image_path = path.parent().unwrap().join(format!("images/{id}.png"));
        assert!(image_path.exists());

        store.clear_unpinned();
        assert!(!image_path.exists());

        let _ = fs::remove_dir_all(path.parent().unwrap());
    }
}
