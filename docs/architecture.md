# Architecture

Superclip is built with [Tauri](https://tauri.app) v2: a Rust backend drives a
React + Tailwind frontend through Tauri's command API.

## High-level layout

```
src/                      React frontend
  App.tsx                 History list + wiring to backend commands
  components/             SearchBar, Clipboard list, Preview pane, Footer
  updater.ts              Channel selection, update check → install flow
  UpdaterApp.tsx          Update panel (status, progress, channel switch)
src-tauri/
  src/
    main.rs               Thin entrypoint
    lib.rs                Tauri builder, tray, hotkey, window lifecycle
    commands.rs           Tauri commands and clipboard write/paste flow
    updater.rs            Update channel + check/install commands
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
  heuristically, entries dedupe, and history caps at 100 items. The selected
  update channel lives in the same file.
- **Commands** (`commands.rs`) — `get_history`, `paste_item`, `toggle_pin`,
  `clear_history`, `get_image` are exposed to the frontend via
  `invoke_handler`; `updater.rs` adds `get_channel`, `set_channel`,
  `check_update`, and `install_update`. `get_image` returns the stored PNG as
  base64 for the preview pane, reading and encoding via `spawn_blocking` so a
  large screenshot never stalls the main thread.
- **Paste** — `paste_item` sets the OS clipboard, then synthesizes a paste
  keystroke targeted at the previously active window:
  - **Linux/X11**: bounded `xdotool windowactivate/windowfocus --sync` calls
    followed by an XTEST `xdotool key ctrl+v` with focus verification and retry.
    Wayland uses `ydotool`/`uinput` when available. Both paths log diagnostics
    to `/tmp/superclip-paste.log`.
  - **Windows**: PowerShell `SendKeys`.
  - **macOS**: `osascript` keystroke via System Events.
- **Tray + hotkey** — the tray menu (Open / Check for Updates / Quit) and a
  global `Ctrl+Shift+V` shortcut toggle the window via `toggle_window`. Showing
  the window emits `palette-shown` (the frontend clears the search field on it)
  and starts a bounded focus retry: `set_focus` is re-issued on the main thread
  once the window reports visible, stopping as soon as focus lands, because
  tao's `set_focus` silently drops requests made before GTK has mapped the
  window.
- **Autostart** — `tauri-plugin-autostart` registers a launch entry with
  `--from-autostart` on first run; the app starts hidden into the tray.

## Frontend

- The picker is a frameless, transparent, always-on-top 750×500 window split
  into a history list and a 300px preview pane.
- `Clipboard.tsx` renders pinned items above recent history and dispatches
  `paste_item`/`toggle_pin` to Rust. Hover selection requires real pointer
  movement, and auto-scroll only follows keyboard navigation, so the mouse and
  the arrow keys can't fight over the selection.
- `Preview.tsx` renders the selected item: images are fetched through
  `get_image` (debounced, with a small data-URL cache) and shown above content
  type, dimensions, size, and copy time; text and code items show their body.
- `App.tsx` focuses the search field on mount and on every window focus, which
  is what makes the palette usable from the keyboard the moment it opens.
- `updater.ts` drives the Rust update commands and reports status into
  `UpdaterApp.tsx`.

## Updates

- `tauri-plugin-updater` verifies manifests with the embedded public key.
  Endpoints are resolved per channel in `updater.rs`: **stable** uses the
  `tauri.conf.json` endpoint (GitHub's `/releases/latest/`, which skips
  prereleases), **beta** overrides it with a rolling `beta` release manifest.
  The channel is persisted in `history.json`, and switching it discards any
  pending update.
- `check_update` parks the resolved `Update` in Tauri state so `install_update`
  can download it later, emitting `updater:progress` as bytes arrive.
- CI signs bundles with a private key in GitHub secrets and `tauri-action`
  uploads signed artifacts + `latest.json` to each GitHub Release. Hyphenated
  tags are published as prereleases, and `beta-channel.yml` mirrors every
  published release's `latest.json` onto the `beta` tag.

## Storage

Local `history.json` metadata plus an `images/` directory in the app data
directory (see `Store`). No accounts, no cloud, no telemetry.
