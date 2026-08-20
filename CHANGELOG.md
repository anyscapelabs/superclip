# Changelog

All notable changes to Superclip are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2-beta.0] - 2026-08-21

### Added
- Image clipboard support: screenshots and copied images are captured
  alongside text, normalized to PNG, stored as files under `images/` in the app
  data directory, and written back to the clipboard losslessly on paste. Copied
  image *files* (a `file://` path or a path with a file payload on the
  clipboard) are captured as images too, keeping their filename.
- Raycast-style preview pane: the palette is now a split view with a 300px
  detail pane on the right. Image items render the picture with `Content type`,
  `Dimensions`, `Size`, and `Copied` metadata beneath it; text and code items
  show their full content in the same pane. Image payloads are fetched lazily
  with a short debounce so arrowing through the list doesn't decode every row,
  and only the last handful stay cached in memory.
- Image items are searchable by filename — fuzzy search now indexes
  `image.name` as well as `text`, so a screenshot's name actually matches.
- Beta channel opt-in: a `Stable` / `Beta` switch in the updater window,
  persisted as `channel` in `history.json`. Stable clients keep polling
  GitHub's `/releases/latest/` manifest (which skips prereleases); beta clients
  poll a rolling `beta` release whose `latest.json` CI overwrites on every
  publish, stable or pre-release, so the beta channel is a superset and can't
  strand a tester on an old build.
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
- The palette window grew from 640×420 to 750×500 to match Raycast's
  proportions and give the preview pane room.
- The updater window was rebuilt as an interactive 420×320 panel: version
  header, status row with a live download progress bar, channel switch, and a
  `Restart now` action once an update is installed. It no longer auto-hides
  after 2.5s, so `Escape` and a close button were added to dismiss it.
- Update checks moved out of `@tauri-apps/plugin-updater`'s JS API into Rust
  commands (`check_update` / `install_update`). The JS `check()` cannot
  override endpoints, which per-channel manifests require; the Rust
  `updater_builder()` can, and only the beta channel overrides — stable still
  resolves from `tauri.conf.json`, so the endpoint isn't duplicated.
- The search field clears on every open, so the palette always starts fresh
  instead of re-showing the last query.
- Release CI marks any hyphenated tag (`v0.1.2-beta.0`) as a GitHub
  prerelease, which is what keeps beta builds out of the stable channel.
- Documented Linux Wayland dependencies (`wl-clipboard`, `ydotool`) in the
  README, including the `ydotoold` daemon and `input` group setup.

### Fixed
- The palette did not take keyboard focus when opened with the hotkey — you
  had to click it before arrows and `Enter` did anything. Three separate causes:
  tao's `set_focus` silently drops the request when the `GtkWindow` isn't
  mapped yet (and `show()` is queued, so it never is); the workaround retry was
  a single blind 60ms sleep that also called GTK off the main thread; and
  nothing in the frontend ever took DOM focus. Focus is now retried on the main
  thread once the window reports visible, stopping as soon as it lands, and the
  search field takes focus on every show. This also fixes type-to-search, which
  had never worked — the input was never focused.
- Mouse hover and scrolling fought each other in the list. Selection was driven
  by `mouseenter` while every selection change called `scrollIntoView`, so
  arrowing scrolled the list, slid a row under the stationary cursor, fired
  `mouseenter`, and yanked the selection to it. Hover selection now requires
  real pointer movement (`mousemove` with changed coordinates, which filters the
  synthetic events browsers emit after a scroll), and auto-scroll only follows
  keyboard navigation. Wheel-scrolling no longer moves the selection.
- `get_image` ran its file read and base64 encode on the main thread, hitching
  the UI on large screenshots. It now runs via `spawn_blocking`.
- The updater's download progress always reported 0% — the `Progress` handler
  ignored the chunk sizes it was given. Progress is now accumulated against the
  content length in Rust and emitted as a real percentage.
- Removed the `copy_item` command, which was registered but had no callers.
- A stale or second instance holding the `Ctrl+Shift+V` hotkey no longer makes
  the app panic at startup — the global shortcut registration now degrades
  gracefully and the app keeps running in the tray instead of crashing.
- Pasting felt sluggish (multi-second stalls in the worst case) because the
  500 ms clipboard watcher called `get_image()` on every cycle and the X11
  backend fell through to probing five raster formats via subprocess while
  holding the same clipboard mutex the paste path needs. The watcher now only
  probe images when the clipboard holds no text (pure image copies), and the
  X11 fallback asks the owner for its advertised targets once instead of
  downloading every format blind.
- Image paste no longer round-trips through a PNG decode + re-encode (~300 ms
  on large screenshots). The X11 backend now hands the stored PNG straight to
  a detached `xclip` serving the `image/png` target, verifying ownership was
  claimed before returning; `arboard` remains as a fallback if `xclip` is
  missing.
- Fixed paste landing at all in Chromium, Electron, and most GTK apps: the
  synthetic Ctrl+V was sent via `xdotool key --window`, which uses XSendEvent —
  those apps discard synthetic key events as untrusted, so nothing pasted even
  though xdotool reported success. The paste key now goes through xdotool's
  XTEST path (indistinguishable from a real key press) after focus is verified.
- Clicking a history item no longer freezes the palette while the clipboard
  write runs: the window hides before the image-ownership poll instead of
  after, so the click always closes it instantly.
- The detached `xclip` used for image paste is now reaped from a background
  thread, so old instances don't accumulate as zombie processes after their
  clipboard ownership is superseded.
- Pasting or copying an existing item no longer re-serializes the entire
  `history.json` (every item's inline text body) to disk on every click —
  `upsert` treated each re-paste as a brand-new capture and paid a full
  serialize + write for a no-op. Items are now bumped in place via a cheap
  `bump()` that skips the write when the item is already at the top, and the
  store mutation runs after the clipboard lock is released so a slow disk
  write can't hold up the watcher.
- `xdotool windowactivate --sync`/`windowfocus --sync` and the `key` send now
  run under a 1.5s watchdog. `--sync` blocks until the WM acknowledges and has
  no native timeout; a WM/app that never replies used to hang the paste thread
  forever instead of failing bounded and letting the focus-verification retry
  recover.

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