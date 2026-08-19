use super::ClipboardBackend;

// --- TEXT clipboard path -----------------------------------------------------
// Dedicated writer for the text half of the clipboard. It owns its own backend
// instance (arboard on X11, wl-copy on Wayland) so its writes can never contend
// with the image path or the watcher for a shared lock or X connection. This is
// the structural separation that keeps "fix text, break image" and vice versa
// from happening: each write path has its own lock and its own resources.

/// Sets the clipboard's text content for a paste/copy. Called under a dedicated
/// Mutex managed by lib.rs, never the shared clipboard backend's lock.
pub struct TextClipboard {
    io: Box<dyn ClipboardBackend>,
}

impl TextClipboard {
    pub fn new() -> Self {
        Self {
            io: super::detect_backend(),
        }
    }

    /// Puts `text` on the clipboard. The backend's set_text is sub-millisecond
    /// on both X11 (arboard) and Wayland (wl-copy), so this never blocks for
    /// long enough to matter.
    pub fn write(&mut self, text: &str) -> Result<(), String> {
        self.io.set_text(text)
    }
}