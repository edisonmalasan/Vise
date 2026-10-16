# Vise Transcoder

Vise Transcoder is a clean-room media transcoder implemented as a Rust workspace. It currently provides a Rust core crate, a command-line interface, a Tauri desktop scaffold, and tests verified against local media fixtures.

## Requirements

- Rust stable toolchain with Cargo
- ffmpeg and ffprobe 9.x or compatible
- Microsoft Visual Studio C++ Build Tools on Windows when native tooling is needed

## Build and test

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## Run the CLI

```powershell
cargo run -p vise-cli -- --help
```

## Documentation

- `AGENTS.md` — project guidance and working rules
- `docs/ROADMAP.md` — milestone and feature status
- `docs/SETUP.md` — local setup and verification guide
- `re/docs/` — reverse-engineering reference notes, ignored by Git

## Notes

- The desktop app under `apps/vise-desktop` is currently a scaffold and uses placeholder npm scripts.
- Media features rely on an externally installed `ffmpeg`/`ffprobe` unless a native implementation is added later.