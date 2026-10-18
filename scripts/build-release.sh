#!/usr/bin/env bash
# Build a release, optionally with the media tools bundled beside it.
#
# The tooling is not downloaded here: fetch-media-tools.sh does that and writes
# the manifest a release is checked against. This script only decides whether
# the tools are present and enabled, and refuses a bundled build without them
# rather than shipping one that silently falls back to the machine's encoder.
#
# usage: ./scripts/build-release.sh [--bundled] [--platform <os-arch>]

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

bundled=0
platform=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundled) bundled=1; shift ;;
    --platform) platform="${2:-}"; shift 2 ;;
    -h|--help) sed -n '2,9p' "$0"; exit 0 ;;
    *) echo "build-release: unknown argument '$1'" >&2; exit 1 ;;
  esac
done

host_platform() {
  local os arch
  case "$(uname -s)" in
    Darwin) os=darwin ;;
    Linux) os=linux ;;
    MINGW*|MSYS*|CYGWIN*) os=windows ;;
    *) os=other ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=aarch64 ;;
    *) arch=other ;;
  esac
  echo "$os-$arch"
}

platform="${platform:-$(host_platform)}"
tools="$root/apps/vise-desktop/src-tauri/vendor/ffmpeg/$platform"

if [[ "$bundled" -eq 1 ]]; then
  if [[ ! -d "$tools" ]]; then
    echo "build-release: a bundled build was asked for but no tools are fetched" >&2
    echo "  run ./scripts/fetch-media-tools.sh $platform first" >&2
    exit 1
  fi
  "$root/scripts/verify-media-tools.sh" "$platform"
  echo "build-release: bundling the tools from $tools"
else
  echo "build-release: not bundling; the build will use the machine's own ffmpeg"
  echo "  (this is a development build and will say so in its diagnostics)"
fi

# The version lives in one place so the application, the bundle, and the
# changelog cannot disagree.
version="$(cat "$root/VERSION")"
echo "build-release: version $version"

cargo build --workspace --release

worker="$root/target/release/vise-worker"
desktop="$root/target/release/vise-desktop"
for binary in "$worker" "$desktop"; do
  if [[ ! -x "$binary" ]]; then
    echo "build-release: expected $binary to have been built" >&2
    exit 1
  fi
done

stage="$root/target/release-package/$platform"
rm -rf "$stage"
mkdir -p "$stage"

if [[ "$bundled" -eq 1 ]]; then
  staged="$root/apps/vise-desktop/src-tauri/vendor/ffmpeg/staged"
  if [[ ! -d "$staged" ]]; then
    echo "build-release: nothing was staged for bundling" >&2
    echo "  run ./scripts/fetch-media-tools.sh $platform first" >&2
    exit 1
  fi
  mkdir -p "$stage/media"
  cp "$staged"/* "$stage/media/"
fi

cp "$desktop" "$stage/"
cp "$worker" "$stage/"

cat > "$stage/BUILD-INFO.txt" <<EOF
Vise Transcoder $version
platform: $platform
bundled media tooling: $([[ "$bundled" -eq 1 ]] && echo yes || echo no)
built from: $(git rev-parse --short HEAD 2>/dev/null || echo unknown)
EOF

echo "build-release: staged in $stage"
echo "  contents:"
( cd "$stage" && find . -type f | sed 's|^\./|    |' | sort )

# The Tauri bundle needs the worker beside the application too: the window
# resolves it from the executable's own directory, so an installer without it
# would fall back to running the pipeline in-process, which is exactly what
# process isolation exists to avoid.
sidecar="$root/apps/vise-desktop/src-tauri/vendor/sidecar"
rm -rf "$sidecar"
mkdir -p "$sidecar"
worker_name="vise-worker"
[[ "$(uname -s)" == "MINGW"* || "$(uname -s)" == "CYGWIN"* ]] && worker_name="vise-worker.exe"
cp "$worker" "$sidecar/$worker_name"
echo "build-release: worker staged for bundling in $sidecar"