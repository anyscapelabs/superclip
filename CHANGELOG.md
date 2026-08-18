# Changelog

All notable changes to Superclip are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2-beta.0] - 2026-08-18

### Added
- Keyboard shortcut `Ctrl+P` / `Cmd+P` pins the currently selected item
  (the footer hint already advertised it, but it was never wired up).
- Native Wayland session support (GNOME, KDE, Sway, …): the clipboard layer is
  now a `ClipboardBackend` abstraction chosen at startup from
  `XDG_SESSION_TYPE`. Wayland sessions use `wl-copy`/`wl-paste` for clipboard
  read/write and `ydotool` (via `uinput`) for synthetic paste; X11 and all
  other OSes keep the existing `arboard` + `xdotool` path unchanged.
- `ydotool` auto-paste degrades gracefully when the daemon/tool isn't
  installed — the item is copied to the clipboard and the app asks the user to
  press `Ctrl+V` manually instead of failing silently.

### Changed
- Documented Linux Wayland dependencies (`wl-clipboard`, `ydotool`) in the
  README, including the `ydotoold` daemon and `input` group setup.

### Fixed
- A stale or second instance holding the `Ctrl+Shift+V` hotkey no longer makes
  the app panic at startup — the global shortcut registration now degrades
  gracefully and the app keeps running in the tray instead of crashing.

## [0.1.1] - 2026-08-08

### Added
- Auto-start with the OS on login (via `tauri-plugin-autostart`); the app
  launches quietly into the system tray, hidden and ready.
- Updater signing for all platforms; `latest.json` served from GitHub Releases.

### Fixed
- Linux `.deb`/`.desktop` metadata now reports `Categories=Utility`, a real
  description, and the Superclip icon (previously showed a generic Tauri app
  with no category).
- App identity string in the about/metadata is now "Anyscape Labs PLC".
- macOS builds compile again: the nonexistent `app.activate()` call was
  replaced with Tauri v2's `set_activation_policy`, and the
  `macos-private-api` feature is enabled.

### Changed
- `productName` is now `Superclip` everywhere (bundles, window, tray tooltip).
- Cargo and bundle descriptions updated from the placeholder "A Tauri App".

## [0.1.0] - 2026-08-08

### Added
- Local-first clipboard history: captures text copies automatically,
  deduplicated, capped at 100 items (oldest drop off).
- Global hotkey `Ctrl+Shift+V` toggles the picker from any app.
- Fuzzy search across history as you type.
- Pin favorites so they never scroll off; pinned items render in their own
  section above recent history.
- Reliable paste: synthetic `Ctrl+V` with focus verification and retry that
  targets the window you were in (works in browsers, IDEs, and other apps).
- System tray menu: Open Superclip, Check for Updates…, Quit.
- Auto-updater with a dedicated update-check card that downloads, installs,
  and relaunches automatically.
- Local-only storage — everything lives in a single local JSON file; no
  accounts, no cloud, no telemetry.

### Changed
- Renamed the app from ClipMate to Superclip (repo, crate, window, branding).
- Rebuilt the UI and documentation site around the Superclip brand.
- Paste reliability hardened with focus-verification retries and a
  `/tmp/superclip-paste.log` diagnostic log.