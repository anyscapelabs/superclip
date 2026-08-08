# Architecture

Superclip is built with [Tauri](https://tauri.app) v2: a Rust backend drives a
React + Tailwind frontend through Tauri's command API. The whole app is under
~1,000 lines of source.

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
    lib.rs                Tauri builder, tray, hotkey, paste engine
    watcher.rs            Clipboard polling thread (arboard)
    store.rs              History store + text/code detection
    detect.rs             Content heuristics (kind detection)
  tauri.conf.json         Window, bundle, updater, autostart config
  capabilities/default.json  Permissions for the webview
```

## Backend

- **Watcher** (`watcher.rs`) — a background thread polls the OS clipboard via
  [`arboard`](https://crates.io/crates/arboard) every ~500ms and hashes each
  entry to detect changes without re-writing identical text.
- **Store** (`store.rs`) — a shared `Mutex<Store>` in Tauri state persists
  history to `history.json` under the app data directory. Text vs. code is
  classified heuristically; entries dedupe; history caps at 100 items.
- **Commands** (`lib.rs`) — `get_history`, `copy_item`, `paste_item`,
  `toggle_pin`, `clear_history` are exposed to the frontend via
  `invoke_handler`.
- **Paste** — `paste_item` sets the OS clipboard, then synthesizes a paste
  keystroke targeted at the previously active window:
  - **Linux**: `xdotool windowactivate/windowfocus --sync` + a verified
    `xdotool key --window <id> ctrl+v` with bounded focus-retry, and a
    `/tmp/superclip-paste.log` for diagnostics.
  - **Windows**: PowerShell `SendKeys`.
  - **macOS**: `osascript` keystroke via System Events.
- **Tray + hotkey** — the tray menu (Open / Check for Updates / Quit) and a
  global `Ctrl+Shift+V` shortcut toggle the window via `toggle_window`.
- **Autostart** — `tauri-plugin-autostart` registers a launch entry with
  `--from-autostart` on first run; the app starts hidden into the tray.

## Frontend

- The picker is a frameless, transparent, always-on-top 640×420 window.
- `Clipboard.tsx` renders pinned items above recent history and dispatches
  `copy_item`/`paste_item`/`toggle_pin` to Rust.
- `updater.ts` wraps `@tauri-apps/plugin-updater`'s `check`→`downloadAndInstall`
  →`relaunch` and reports status into `UpdaterApp.tsx`.

## Updates

- `tauri-plugin-updater` checks the manifest at the configured endpoint
  (GitHub `latest.json`) using the embedded public key.
- CI signs bundles with a private key in GitHub secrets and `tauri-action`
  uploads signed artifacts + `latest.json` to each GitHub Release.

## Storage

Single local JSON file in the app data directory (see `Store`). No accounts,
no cloud, no telemetry.