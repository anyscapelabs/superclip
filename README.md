# ClipMate

A clipboard manager you can actually read the source of. Under 1,000 lines, no telemetry, no cloud — just your clipboard, remembered.

ClipMate sits quietly in your system tray. Copy things as normal. Hit a hotkey, and your recent clipboard history shows up in a small window. Click an item to copy it again. That's the whole app.

## Why

Most clipboard managers are either bloated Electron apps or ask for cloud sync you never wanted. ClipMate is built with [Tauri](https://tauri.app), so it's a native app with a tiny footprint — around 10–15MB, low idle RAM, no background telemetry. Everything is stored in a local JSON file on your machine and nothing ever leaves it.

## Features

- **Background clipboard watcher** — captures text copies automatically, deduplicated
- **Global hotkey** — `Ctrl+Shift+V` (configurable) opens the history from any app
- **Search** — type to filter your history instantly
- **Click to copy back** — click an item, it's back on your clipboard
- **Pin favorites** — keep frequently used snippets from scrolling off
- **Auto-purge** — history caps at 100 items by default, oldest items drop off
- **Local-only** — no accounts, no sync, no network calls
- **Cross-platform** — Windows, Linux, and macOS

## Install

### From a release
Download the latest build for your OS from [Releases](../../releases) and run the installer.

### From source
Requires [Rust](https://www.rust-lang.org/tools/install) and [Node.js](https://nodejs.org/) / [Bun](https://bun.sh).

```bash
git clone https://github.com/anyscapelabs/clipmate.git
cd clipmate
bun install
bun run tauri dev      # run in development
bun run tauri build    # produce a release binary
```

## Usage

1. Launch ClipMate — it starts in your system tray.
2. Copy something, anywhere.
3. Press `Ctrl+Shift+V` to open your history.
4. Click an item to paste it back, or press Enter. Arrows navigate, `Ctrl+X` clears.
5. Right-click the tray icon to open ClipMate or quit.

## How it works

- A background thread polls the OS clipboard every ~500ms using [`arboard`](https://crates.io/crates/arboard) and hashes each entry to detect changes.
- New entries are appended to a JSON history file in the app's local data directory.
- Text vs code is detected heuristically in the backend.
- The frontend is plain React + Tailwind talking to the Rust backend through Tauri's command API.
- A global shortcut (via `tauri-plugin-global-shortcut`) toggles the window without needing focus.

## Roadmap

- [ ] Image clipboard support (screenshots, copied images)
- [ ] Optional encryption at rest for sensitive entries
- [ ] Per-item expiry (auto-delete after N minutes)
- [ ] Light/dark/system theme support
- [ ] Configurable hotkey

## Privacy

ClipMate never sends data anywhere. There is no analytics, no update-check ping, no cloud sync. Your clipboard history lives in a single local file you can open, inspect, or delete at any time.

## Contributing

Pull requests are welcome. For anything beyond a small fix, please open an issue first to discuss the change. Keep additions consistent with the project's goal: small, readable, and dependency-light.

## License

[MIT](LICENSE)
