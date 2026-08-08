# Changelog

All notable changes to Superclip are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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