# Usage

Superclip is designed to stay out of your way: you copy things normally and
Superclip quietly remembers them.

## First launch

Superclip starts automatically with your OS and sits in the **system tray**.
Nothing pops up — it's ready before you notice it. Right-click the tray icon
to open the menu:

- **Open Superclip** — show the history window.
- **Check for Updates…** — manually look for a new version (installs and
  relaunches when one is found).
- **Quit** — fully exit Superclip.

## Everyday workflow

1. **Copy something** in any app, as usual.
2. **Press `Ctrl+Shift+V`** anywhere — the history window opens centered on
   your screen, on top of everything.
3. **Type to search** — history is filtered as you type (fuzzy matching).
4. **Pick an item**:
   - **Click** or press **Enter** to paste it into the app you were in.
   - Hover an item and click the **pin icon** to keep it forever (pinned items
     live in their own section above recent history).
5. The window closes after you paste.

## History

- Everything you copy is captured automatically, deduplicated, and stored
  locally in a single JSON file.
- History is capped at **100 items**; the oldest entries drop off first.
- **Pinned** items never drop off. Unpinned items can be cleared with the
  clear action in the UI.

## Tips

- Superclip remembers *text* copies. Image and file clipboard content is not
  captured yet (see [Roadmap](../README.md#roadmap)).
- If you open Superclip with a keyboard shortcut and it doesn't appear, make
  sure your desktop session is running the X11 backend.
- Everything stays on your machine — no accounts, no cloud, no telemetry.

## Shortcuts

| Action | Shortcut |
| --- | --- |
| Open / close history | `Ctrl+Shift+V` |
| Paste selected item | `Enter` |
| Pin / unpin item | Hover the item, click the pin |

The global hotkey is registered with `tauri-plugin-global-shortcut` and can be
changed in the source before building.
