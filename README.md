# Vise Transcoder

Vise Transcoder is a clean-room media transcoder implemented as a Rust workspace.
It provides a Rust core, a command-line interface, a Tauri desktop application, and
a worker process that runs the pipeline on the user's behalf.

The desktop application never runs the pipeline inside its own window. A separate
worker process serves a token-authenticated loopback API, so a crash in the encoder
fails one job rather than the application, and the window stays responsive while a
long encode runs.

## Requirements

- Rust stable toolchain with Cargo
- ffmpeg and ffprobe 9.x, or the copies a release build carries
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

## Run the desktop application

```powershell
cargo run -p vise-desktop
```

## Build a release

A release carries its own copy of the media tools, so that the encoders and
filters the application offers are always the ones it was tested against. The
tools are pinned per platform by digest.

```bash
./scripts/fetch-media-tools.sh          # fetch and verify the pinned tools
./scripts/build-release.sh --bundled    # build and stage a package
```

The Linux package is built in a container, because the desktop window needs
libraries that a Windows machine cannot provide:

```bash
./scripts/build-linux.sh --bundled
```

Continuous integration builds and packages all three platforms; see
`.github/workflows/release.yml`.

## Documentation

- `AGENTS.md` — project guidance and working rules
- `docs/ROADMAP.md` — the feature roadmap and its verification state
- `docs/CONTEXT.md` — a router to the authoritative source for each kind of question
- `docs/SETUP.md` — local setup and verification guide
- `docs/media-tooling.md` — how the bundled media tools are found, pinned, and checked
- `re/docs/` — reverse-engineering reference notes, ignored by Git

## Notes

- The media tools are found beside the application first, then on the machine's
  search path. A build with nothing bundled says so rather than behaving quietly
  differently from a shipped one.
- A hardware encoder is only offered when the machine actually has the device
  it needs, because the encoder being compiled into the tool says nothing about
  the hardware being present.
- A DVD or a Blu-ray is read as a disc rather than as a file, and a build whose
  tools cannot read one says which capability is missing.
