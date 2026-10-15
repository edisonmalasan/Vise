# Reproducible Build Process

This document describes how to produce reproducible Vise builds.

## Prerequisites
- Rust toolchain (pinned via `rust-toolchain.toml` when available).
- Cargo workspace at repository root.

## Build
```sh
cargo build --release
```
Outputs land in `target/release/`.

## Reproducibility notes
- Release profile uses `lto = "thin"`, `codegen-units = 1`, and `strip = "symbols"`.
- Avoid environment-specific paths in build scripts.
- Record the toolchain version and commit hash in release notes.
- Prefer a clean checkout and `cargo build --release --locked`.

## Verification
```sh
cargo test
cargo build --release --locked
```
