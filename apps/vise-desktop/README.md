# Vise Desktop

The Tauri shell and the web page it serves. There is no build step: the page
is plain HTML, CSS, and JavaScript, and `src-tauri/tauri.conf.json` points the
window at this directory.

## Layout

| Path | Purpose |
|---|---|
| `ui/index.html` | Page structure |
| `ui/styles.css` | One stylesheet, light and dark |
| `ui/app.js` | Wiring only: commands in, painted objects out |
| `src-tauri/src/main.rs` | Tauri commands, queue, logs |
| `src-tauri/src/sources.rs` | Source validation and the recent source list |
| `src-tauri/src/viewmodel.rs` | Everything the page renders |
| `src-tauri/src/scanflow.rs` | Source, scan, title, range, and track state |

## Boundaries

- The window never runs the pipeline. Scanning and encoding happen in the
  worker process reached over its loopback JSON API; the page talks only to
  the Tauri commands.
- The page holds no media logic. Durations, resolutions, language labels, range
  validity, and track projection are built in `viewmodel.rs` and covered by
  unit tests, so what the user reads is testable without a window.
- Scanning does not block a command. `begin_scan` hands the work to the worker
  and `scan_status` polls it, which keeps the window painting progress.

## Build and test

- `cargo build --workspace`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`