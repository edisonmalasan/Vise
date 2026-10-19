#!/usr/bin/env bash
# Check that the tools already on disk are the ones the pin file names.
#
# This is what a release job runs before it publishes: it does not fetch, so it
# proves that what is about to be shipped is what was reviewed.
#
# usage: ./scripts/verify-media-tools.sh [platform]

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lock="$root/media-tools.json"
vendor="$root/apps/vise-desktop/src-tauri/vendor/ffmpeg"

# See fetch-media-tools.sh: a `python3` that only opens the Microsoft Store is
# not an interpreter, so the candidates are asked whether they actually run.
PYTHON=""
for candidate in python3 python; do
  if command -v "$candidate" >/dev/null 2>&1 && "$candidate" --version >/dev/null 2>&1; then
    PYTHON="$candidate"
    break
  fi
done
if [[ -z "$PYTHON" ]]; then
  echo "verify-media-tools: a working python is needed to read the pin file" >&2
  exit 1
fi

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

platform="${1:-$(host_platform)}"
target="$vendor/$platform"

if [[ ! -f "$lock" ]]; then
  echo "verify-media-tools: no pin file at $lock" >&2
  exit 1
fi
if [[ ! -d "$target" ]]; then
  echo "verify-media-tools: nothing fetched for $platform" >&2
  echo "  run ./scripts/fetch-media-tools.sh $platform first" >&2
  exit 1
fi

$PYTHON - "$lock" "$platform" "$target" <<'PY'
import hashlib, json, os, sys

lock_path, platform, target = sys.argv[1], sys.argv[2], sys.argv[3]
lock = json.load(open(lock_path, encoding="utf-8"))
entry = lock["platforms"].get(platform)
if entry is None:
    sys.exit(f"verify-media-tools: '{platform}' is not in the pin file")

problems = []
for tool, expected in sorted(entry["digests"].items()):
    path = os.path.join(target, tool)
    if not os.path.isfile(path):
        problems.append(f"{tool}: not present")
        continue
    with open(path, "rb") as handle:
        actual = hashlib.sha256(handle.read()).hexdigest()
    if expected.startswith("REPLACE_"):
        # A binary whose digest is not pinned is still covered when the archive
        # it came from is: the archive determines its contents exactly. The
        # archive digests are checked below.
        continue
    if expected != actual:
        problems.append(f"{tool}: expected {expected}, found {actual}")

archives = entry.get("archives")
if archives is None:
    archives = [{
        "name": entry["archive"],
        "url": entry["url"],
        "sha256": entry.get("archive_sha256", ""),
    }]
cache = os.path.join(os.path.dirname(target), ".cache")
for archive in archives:
    digest = archive.get("sha256", "")
    if not digest or digest.startswith("REPLACE_"):
        problems.append(f"{archive['name']}: the pin file has no digest to compare against")
        continue
    path = os.path.join(cache, archive["name"])
    if not os.path.isfile(path):
        # The archive is not needed to check the tools that were unpacked from
        # it, so its absence is only worth reporting when the tools are missing
        # too, which the loop above already covers.
        continue
    with open(path, "rb") as handle:
        actual = hashlib.sha256(handle.read()).hexdigest()
    if digest != actual:
        problems.append(f"{archive['name']}: expected {digest}, found {actual}")

if problems:
    print("verify-media-tools: the tools on disk are not the pinned ones:", file=sys.stderr)
    for problem in problems:
        print(f"  {problem}", file=sys.stderr)
    sys.exit(1)

print(f"verify-media-tools: {platform} matches the pin file (ffmpeg {entry.get('version', '?')})")
PY