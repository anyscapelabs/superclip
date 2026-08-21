# Superclip

> A clipboard manager you can actually read the source of. Under 1,000 lines, no telemetry, no cloud — just your clipboard, remembered.

Superclip sits quietly in your system tray. Copy things as normal. Hit a hotkey, and your recent clipboard history shows up in a small window. Search, pin, and paste anything you've copied — instantly.

If Superclip saves you a keystroke, give it a star ⭐ — it helps more people find it.

![Superclip demo](public/demo.gif)

## Install

Download the latest build for your OS from [Releases](https://github.com/anyscapelabs/superclip/releases) and run the installer (Windows `.exe`, macOS `.dmg`, Linux AppImage/DEB/RPM).

**[Install guide →](docs/usage.md)**

### Linux dependencies (Wayland)

On Wayland sessions (GNOME, KDE, Sway, …) Superclip shells out to a couple of
standard tools for clipboard access and synthetic paste, because the Wayland
security model forbids apps from injecting input or reading the selection
directly.

```bash
# Debian / Ubuntu / Mint
sudo apt install wl-clipboard ydotool

# Enable the ydotool daemon so Superclip can auto-paste
sudo systemctl enable --now ydotoold
sudo usermod -aG input $USER   # log out and back in after this
```

`ydotool` setup is **optional** — clipboard history still works without it,
you'll just need to press `Ctrl+V` yourself after picking an item. X11 packages
include `xclip` (for lossless image serving) and `xdotool` (for auto-paste);
they are declared for Debian packages and should be installed by other Linux
package managers when building from source.

## Features

- **Background clipboard watcher** — captures text and image copies automatically, deduplicated
- **Global hotkey** — `Ctrl+Shift+V` opens the history from any app
- **Instant search** — fuzzy filter across your whole history as you type, images included by filename
- **Image support** — screenshots, copied pictures, and image files are kept as PNGs and paste back at full quality
- **Preview pane** — the selected item is previewed beside the list: images with their dimensions and size, text and code with their full content
- **Paste anywhere** — paste the selected item back into the app you were in
- **Pin favorites** — keep frequently used snippets from scrolling off, pinned above recent history
- **Auto-purge** — history caps at 100 items by default, oldest items drop off
- **Auto-updates** — signed, checked on demand from the tray, with an opt-in beta channel
- **Runs at login** — starts quietly into the system tray
- **Local-only** — no accounts, no sync, no network calls
- **Cross-platform** — Windows, Linux, and macOS
- **Tiny footprint** — native Tauri app, ~10–15MB, low idle RAM

## Why

Most clipboard managers are either bloated Electron apps or push cloud sync you never wanted. Superclip is built with [Tauri](https://tauri.app): a native app with a ~10–15MB footprint, low idle RAM, and no background telemetry. History metadata lives in a local JSON file and image payloads live beside it; neither leaves your machine.

## From source

Requires [Rust](https://www.rust-lang.org/tools/install) and [Bun](https://bun.sh).

```bash
git clone https://github.com/anyscapelabs/superclip.git
cd superclip
bun install
bun run tauri dev      # run in development
bun run tauri build    # produce a release binary
```

## Usage

1. Launch Superclip — it starts quietly in your system tray (and on login).
2. Copy something, anywhere.
3. Press `Ctrl+Shift+V` to open your history.
4. Start typing to search; use arrows to navigate, Enter to paste.
5. Hover an item and click the pin, or press `Ctrl+P`, to keep it forever.
6. Right-click the tray icon to open Superclip, check for updates, or quit.

**[Full usage guide →](docs/usage.md)**

## How it works

- A background thread polls the OS clipboard every ~500ms and hashes each entry to detect changes — via `arboard` plus bounded `xclip` fallbacks on X11, and `wl-paste` on Wayland.
- New entries are appended to a JSON history file in the app's local data directory.
- Paste focuses the window you were in and synthesizes `Ctrl+V` (via `xdotool`/XTEST on X11, `ydotool`/`uinput` on Wayland, SendKeys on Windows, System Events on macOS) with focus verification and retry.
- Text vs code is detected heuristically in the backend.
- Signed updates are checked on demand; stable pulls GitHub's latest release manifest, and the opt-in beta channel pulls a rolling pre-release manifest instead.
- The frontend is plain React + Tailwind talking to the Rust backend through Tauri's command API.
- A global shortcut (via `tauri-plugin-global-shortcut`) toggles the window without needing focus.

**[Architecture →](docs/architecture.md)**

## Roadmap

- [x] Clipboard history with search and pinning
- [x] Reliable paste into the app you came from
- [x] Signed auto-updates
- [x] Auto-start into the tray
- [x] Image clipboard support (screenshots, copied images)
- [ ] Optional encryption at rest for sensitive entries
- [ ] Per-item expiry (auto-delete after N minutes)
- [ ] Terminal paste (X11 PRIMARY / middle-click)
- [ ] Configurable hotkey

## Privacy

Superclip never sends data anywhere. There is no analytics, no cloud sync. Your clipboard history lives in a single local file you can open, inspect, or delete at any time. The only network call is the optional auto-update check.

## Contributing

Pull requests are welcome. For anything beyond a small fix, please open an issue first to discuss the change. Keep additions consistent with the project's goal: small, readable, and dependency-light.

**[Contributing guide →](docs/CONTRIBUTING.md)**

## License

[MIT](LICENSE)

## Changelog

See [CHANGELOG.md](CHANGELOG.md).
