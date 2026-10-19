# Build a Vise release inside a Linux container.
#
# The desktop application needs a webview toolkit and a handful of native
# libraries that a Windows machine cannot provide, so the Linux package is built
# here rather than cross-compiled. The Windows package is built on the host
# instead, because the same reasoning does not apply to it.
#
# usage: ./scripts/build-linux.sh [--bundled]
#
# The source is mounted rather than copied so a build uses the working tree as it
# stands; only the target directory and the fetched tools are written back to the
# host, and they are already ignored there.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

bundled=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --bundled) bundled=1; shift ;;
    -h|--help) sed -n '2,11p' "$0"; exit 0 ;;
    *) echo "build-linux: unknown argument '$1'" >&2; exit 1 ;;
  esac
done

image="vise-build-linux:stable"
container_root=/build

echo "build-linux: preparing $image"

# Git Bash rewrites any argument that looks like a path before the program sees
# it, so a container working directory of /src arrives as a path inside the Git
# installation. Turning that off is what lets the container paths mean what they
# say; the host path above is converted by hand instead.
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL="*"

docker build -f scripts/Dockerfile.linux -t "$image" scripts

# Docker Desktop on Windows takes native paths, but this script runs under Git
# Bash on that host, where a path starts with a slash and carries a drive letter
# converted to a leading slash. Left as it is, the container is told to work in
# something like /c/Program Files and refuses to start.
host_root="$root"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    # cygpath ships in the Git installation's usr/bin, which is not always on
    # PATH when this runs from a non-interactive shell, so it is asked for by
    # path rather than by name.
    for candidate in "$(dirname "$(command -v bash)")/../usr/bin/cygpath" /usr/bin/cygpath; do
      if [[ -x "$candidate" ]]; then
        host_root="$("$candidate" -m "$root")"
        break
      fi
    done
    ;;
esac

# Fetched tools are per-platform, and the host holds the Windows ones, so the
# container does its own fetch rather than being handed the wrong binaries.
fetch_args=()
[[ "$bundled" -eq 1 ]] && fetch_args+=(--bundled)

docker run --rm \
  -v "$host_root:/src" \
  -v "vise-linux-cargo:/root/.cargo" \
  -v "vise-linux-target:/build/target" \
  -w /src \
  -e VISE_BUNDLED="$bundled" \
  "$image" \
  bash -c "
    set -euo pipefail
    export PATH=\"/root/.cargo/bin:\$PATH\"
    # The container builds into its own target directory. Sharing the host's
    # would put a Linux binary where the host's Windows one lives, and the two
    # would overwrite each other on every build.
    export CARGO_TARGET_DIR=/build/target
    mkdir -p /build/target
    echo 'build-linux: installing pinned media tools'
    ./scripts/fetch-media-tools.sh linux-x86_64
    ./scripts/verify-media-tools.sh linux-x86_64
    echo 'build-linux: building the release'
    cargo build --workspace --release
    echo 'build-linux: staging the worker beside the application'
    rm -rf apps/vise-desktop/src-tauri/vendor/sidecar
    mkdir -p apps/vise-desktop/src-tauri/vendor/sidecar
    cp target/release/vise-worker apps/vise-desktop/src-tauri/vendor/sidecar/vise-worker
    echo 'build-linux: bundling'
    cd apps/vise-desktop/src-tauri && cargo tauri bundle --bundles deb,appimage
  "

echo "build-linux: artifacts are in target/release/bundle/"
ls -1 target/release/bundle/ 2>/dev/null || true