# Usage

Superclip is designed to stay out of your way: you copy things normally and
Superclip quietly remembers them.

## First launch

Superclip starts automatically with your OS and sits in the **system tray**.
Nothing pops up — it's ready before you notice it. Right-click the tray icon
to open the menu:

- **Open Superclip** — show the history window.
- **Check for Updates…** — open the update panel and look for a new version.
- **Quit** — fully exit Superclip.

## Everyday workflow

1. **Copy something** in any app, as usual.
2. **Press `Ctrl+Shift+V`** anywhere — the history window opens centered on
   your screen, on top of everything, with the search field already focused.
3. **Type to search** — history is filtered as you type (fuzzy matching). The
   field starts empty on every open.
4. **Pick an item**:
   - **Click** or press **Enter** to paste it into the app you were in.
   - Hover an item and click the **pin icon**, or press `Ctrl+P` / `Cmd+P`, to
     keep it forever (pinned items live in their own section above recent
     history).
5. The window closes after you paste.

The right-hand **preview pane** always shows the selected item: images render
as a picture with their content type, dimensions, file size, and copy time;
text and code entries show their full content, which is handy for anything too
long to read in the list.

## History

- Everything you copy is captured automatically, deduplicated, and stored
  locally in a single JSON file.
- **Images** are captured too — screenshots, copied pictures, and copied image
  files. They're stored as PNGs next to the history file and paste back at full
  quality. Image entries can be searched by filename.
- History is capped at **100 items**; the oldest entries drop off first.
- **Pinned** items never drop off. Unpinned items can be cleared with the
  clear action in the UI.

## Updates

Open **Check for Updates…** from the tray to see the update panel. It shows
your current version, checks on demand, reports download progress, and offers
`Restart now` once an update is installed. Press `Escape` to dismiss it.

Two channels are available:

- **Stable** — tested releases only. The default, and the right choice for
  everyday use.
- **Beta** — pre-release builds, so you see new features first and may hit
  rough edges.

Your choice is remembered. Note that switching from Beta back to Stable does
not downgrade you: if you're on `0.1.3-beta.1`, you'll stay there until
`0.1.3` stable ships.

## Tips

- If you open Superclip with a keyboard shortcut and it doesn't appear, make
  sure your desktop session is running the X11 or Wayland dependencies listed
  in the [README](../README.md#linux-dependencies-wayland).
- Everything stays on your machine — no accounts, no cloud, no telemetry. The
  only network call is the update check.

## Shortcuts

| Action | Shortcut |
| --- | --- |
| Open / close history | `Ctrl+Shift+V` |
| Move between items | `↑` / `↓` |
| Paste selected item | `Enter` |
| Pin / unpin selected item | `Ctrl+P` / `Cmd+P` |
| Clear unpinned history | `Ctrl+X` / `Cmd+X` |
| Close the update panel | `Escape` |

The global hotkey is registered with `tauri-plugin-global-shortcut` and can be
changed in the source before building.
