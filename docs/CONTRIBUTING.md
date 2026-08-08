# Contributing

Thanks for wanting to help with Superclip. The project stays small, readable,
and dependency-light on purpose — please keep contributions consistent with
that goal.

## Code of conduct

Be respectful and constructive. Superclip is an open-source project maintained
by Anyscape Labs; harassment or bad-faith reporting won't be tolerated.

## Getting started

Requires [Rust](https://www.rust-lang.org/tools/install) and
[Bun](https://bun.sh).

```bash
git clone https://github.com/anyscapelabs/superclip.git
cd superclip
bun install
bun run tauri dev     # run in development
bun run tauri build   # produce a release binary
```

## Where things live

- `src/` — React + Tailwind frontend
- `src-tauri/src/lib.rs` — Tauri setup, tray, hotkey, commands, paste
- `src-tauri/src/store.rs` — history store
- `src-tauri/src/watcher.rs` — clipboard watcher
- `docs/` — usage, architecture, this guide

## Workflow

1. Open an issue first for anything beyond a small fix.
2. Fork, branch, and open a pull request.
3. Keep one logical change per PR, keep diffs small.
4. Match existing style. No new dependencies unless there's a real reason.

## Checks

Before opening a PR, make sure the release jobs would pass:

```bash
bun run build                          # frontend (tsc + vite)
cargo fmt --all -- --check             # formatting
cargo clippy --all-targets -- -D warnings
```

## Paste plumbing

If you touch the paste path (`send_synthetic_paste`, `activate_window`,
`send_paste_key`), keep the invariants from `lib.rs`:

- exactly one window activation per paste (plus one verified retry)
- use `xdotool key --window <id> ctrl+v` on Linux
- named constants with comments, no new `.unwrap()` in the paste path
- log every external call via `paste_log()` and verify state afterwards

## Documentation

- `CHANGELOG.md` uses Keep a Changelog. Add an entry for user-visible changes.
- Keep `README.md` and `docs/*.md` accurate when behavior changes.