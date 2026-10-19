#!/usr/bin/env bash
# Run the checks inside the Linux build image.
#
# The desktop crate links against the platform's own libraries, so a check that
# only runs on one platform misses whatever the other one would complain about.
# This runs the same checks the release workflow runs, on Linux.
set -uo pipefail
cd /src
export PATH="/root/.cargo/bin:$PATH"
# The container builds into its own target directory. Sharing the host's would
# put a Linux binary where the host's Windows one lives, and the two would
# overwrite each other on every build.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-/build/target}"
mkdir -p "$CARGO_TARGET_DIR"

# The tools are fetched for this platform first, because the staged directory
# holds whatever was fetched last and the build copies it beside the
# application. Checking against another platform's tools would fail in a way
# that looks like a broken build rather than a stale directory.
./scripts/fetch-media-tools.sh linux-x86_64
./scripts/verify-media-tools.sh linux-x86_64

echo "== formatting =="
cargo fmt --all -- --check
echo "== lint =="
cargo clippy --workspace --all-targets -- -D warnings
echo "== tests =="
cargo test --workspace
echo "== done =="