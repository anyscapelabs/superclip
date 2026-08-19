# Architecture

Superclip is built with [Tauri](https://tauri.app) v2: a Rust backend drives a
React + Tailwind frontend through Tauri's command API.

## High-level layout

```
src/                      React frontend
  App.tsx                 History list + wiring to backend commands
  components/             SearchBar, Clipboard list, Footer
  updater.ts              Update check → download → install flow
  UpdaterApp.tsx          Dedicated update status card
src-tauri/
  src/
    main.rs               Thin entrypoint
    lib.rs                Tauri builder, tray, hotkey, window lifecycle
    commands.rs           Tauri commands and clipboard write/paste flow
    watcher.rs            Clipboard polling thread
    store.rs              History store + image-file lifecycle
    detect.rs             Content heuristics (kind detection)
    clipboard/            X11, Wayland, text, and image backends
  tauri.conf.json         Window, bundle, updater, autostart config
  capabilities/default.json  Permissions for the webview
```

## Backend

- **Watcher** (`watcher.rs`) — a background thread polls the OS clipboard every
  ~500ms and hashes each entry to detect changes without re-writing identical
  text. X11 uses [`arboard`](https://crates.io/crates/arboard) with bounded
  `xclip` image fallbacks; Wayland uses `wl-paste`.
- **Store** (`store.rs`) — a shared `Mutex<Store>` in Tauri state persists
  history metadata to `history.json` and image payloads to `images/` under the
  app data directory. Writes are sync-and-replace, unreferenced image payloads
  are collected after a successful mutation, text vs. code is classified
  heuristically, entries dedupe, and history caps at 100 items.
- **Commands** (`commands.rs`) — `get_history`, `copy_item`, `paste_item`,
  `toggle_pin`, `clear_history` are exposed to the frontend via
  `invoke_handler`.
- **Paste** — `paste_item` sets the OS clipboard, then synthesizes a paste
  keystroke targeted at the previously active window:
  - **Linux/X11**: bounded `xdotool windowactivate/windowfocus --sync` calls
    followed by an XTEST `xdotool key ctrl+v` with focus verification and retry.
    Wayland uses `ydotool`/`uinput` when available. Both paths log diagnostics
    to `/tmp/superclip-paste.log`.
  - **Windows**: PowerShell `SendKeys`.
  - **macOS**: `osascript` keystroke via System Events.
- **Tray + hotkey** — the tray menu (Open / Check for Updates / Quit) and a
  global `Ctrl+Shift+V` shortcut toggle the window via `toggle_window`.
- **Autostart** — `tauri-plugin-autostart` registers a launch entry with
  `--from-autostart` on first run; the app starts hidden into the tray.

## Frontend

- The picker is a frameless, transparent, always-on-top 640×420 window.
- `Clipboard.tsx` renders pinned items above recent history and dispatches
  `paste_item`/`toggle_pin` to Rust.
- `updater.ts` wraps `@tauri-apps/plugin-updater`'s `check`→`downloadAndInstall`
  →`relaunch` and reports status into `UpdaterApp.tsx`.

## Updates

- `tauri-plugin-updater` checks the manifest at the configured endpoint
  (GitHub `latest.json`) using the embedded public key.
- CI signs bundles with a private key in GitHub secrets and `tauri-action`
  uploads signed artifacts + `latest.json` to each GitHub Release.

## Storage

Local `history.json` metadata plus an `images/` directory in the app data
directory (see `Store`). No accounts, no cloud, no telemetry.
