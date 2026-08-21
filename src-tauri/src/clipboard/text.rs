use super::ClipboardBackend;

pub struct TextClipboard {
    io: Box<dyn ClipboardBackend>,
}

impl TextClipboard {
    pub fn new() -> Self {
        Self {
            io: super::detect_backend(),
        }
    }

    pub fn write(&mut self, text: &str) -> Result<(), String> {
        self.io.set_text(text)
    }
}
