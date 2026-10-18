#!/usr/bin/env bash
# Fetch the pinned media tools into the desktop crate's vendor/ffmpeg/<platform>/,
# which is where the Tauri bundle is told to look for them.
#
# The lock file is the reproducibility record: it names the version, the upstream
# URL, and the SHA-256 of every binary. Nothing is installed on this machine --
# the tools are unpacked beside the application and found there at run time.
#
# usage: ./scripts/fetch-media-tools.sh [platform]
#   platform defaults to the host, in the form <os>-<arch> used by the lock.

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lock="$root/media-tools.json"
vendor="$root/apps/vise-desktop/src-tauri/vendor/ffmpeg"

# Some Windows environments ship a `python3` alias that only points at the
# Microsoft Store, so the interpreter is chosen by asking each candidate whether
# it actually runs rather than by trusting its name.
PYTHON=""
for candidate in python3 python; do
  if command -v "$candidate" >/dev/null 2>&1 && "$candidate" --version >/dev/null 2>&1; then
    PYTHON="$candidate"
    break
  fi
done
if [[ -z "$PYTHON" ]]; then
  echo "fetch-media-tools: a working python is needed to read the lock file" >&2
  exit 1
fi

host_platform() {
  local os arch
  case "$(uname -s)" in
    Darwin) os=darwin ;;
    Linux) os=linux ;;
    MINGW*|MSYS*|CYGWIN*) os=windows ;;
    *) echo "unsupported operating system: $(uname -s)" >&2; exit 1 ;;
  esac
  case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    arm64|aarch64) arch=aarch64 ;;
    *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
  esac
  echo "$os-$arch"
}

platform="${1:-$(host_platform)}"

read_lock() {
  $PYTHON - "$lock" "$platform" "$1" <<'PY'
import json, sys
lock_path, platform, field = sys.argv[1], sys.argv[2], sys.argv[3]
lock = json.load(open(lock_path, encoding="utf-8"))
entry = lock["platforms"].get(platform)
if entry is None:
    known = ", ".join(sorted(lock["platforms"]))
    sys.exit(f"fetch-media-tools: '{platform}' is not in the lock file (known: {known})")
if field == "all":
    print(json.dumps(entry))
else:
    print(entry[field])
PY
}

if [[ ! -f "$lock" ]]; then
  echo "fetch-media-tools: no lock file at $lock" >&2
  exit 1
fi

url="$(read_lock url)"
archive="$(read_lock archive)"
prefix="$(read_lock member_prefix)"
# The version comes from the pinned archive name rather than a separate field, so
# there is one thing to bump and it cannot disagree with what is downloaded.
version="${archive##*/}"
version="${version#ffmpeg-}"
version="${version%%-build*}"
# The tool list is a JSON array in the lock file; it is read here rather than
# through read_lock so the list itself is decoded, not re-printed as JSON text
# and then split on whitespace.
tools="$($PYTHON - "$lock" "$platform" <<'PY'
import json, sys
lock = json.load(open(sys.argv[1], encoding="utf-8"))
print(" ".join(lock["platforms"][sys.argv[2]]["tools"]))
PY
)"
# The expected digests are read as JSON text rather than through read_lock,
# which would print Python's repr and not something json.loads can read back.
digests_json="$($PYTHON - "$lock" "$platform" <<'PY'
import json, sys
lock = json.load(open(sys.argv[1], encoding="utf-8"))
print(json.dumps(lock["platforms"][sys.argv[2]]["digests"]))
PY
)"
target="$vendor/$platform"

echo "fetch-media-tools: ffmpeg $version for $platform"
echo "  from $url"

mkdir -p "$target" "$vendor/.cache"

archive_path="$vendor/.cache/$archive"
if [[ ! -f "$archive_path" ]]; then
  echo "  downloading"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$url" -o "$archive_path"
  elif command -v wget >/dev/null 2>&1; then
    wget -q "$url" -O "$archive_path"
  else
    echo "fetch-media-tools: neither curl nor wget is available" >&2
    exit 1
  fi
else
  echo "  archive already cached"
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
case "$archive" in
  *.zip) (cd "$work" && unzip -q "$archive_path") ;;
  *.tar.xz|*.txz) (cd "$work" && tar xf "$archive_path") ;;
  *.tar.gz|*.tgz) (cd "$work" && tar xf "$archive_path") ;;
  *) echo "fetch-media-tools: do not know how to unpack $archive" >&2; exit 1 ;;
esac

for tool in $tools; do
  if [[ -n "$prefix" ]]; then
    found="$(find "$work" -type f -path "*$prefix/$tool" -print -quit)"
  else
    found="$(find "$work" -type f -name "$tool" -print -quit)"
  fi
  if [[ -z "$found" ]]; then
    echo "fetch-media-tools: $tool is not in the archive" >&2
    exit 1
  fi
  cp "$found" "$target/$tool"
  chmod +x "$target/$tool" 2>/dev/null || true
  echo "  unpacked $tool"
done

# The manifest is what a release is checked against, so it records what was
# actually fetched rather than what was asked for.
$PYTHON - "$target" "$platform" "$version" "$url" "$digests_json" "$tools" <<'PY'
import hashlib, json, os, sys

target, platform, version, url, digests_json, tools = sys.argv[1:7]
digests = json.loads(digests_json)
manifest = {
    "version": version,
    "platform": platform,
    "source": url,
    "tools": {},
}
problems = []
for tool in tools.split():
    path = os.path.join(target, tool)
    if not os.path.isfile(path):
        problems.append(f"{tool}: missing after unpacking")
        continue
    with open(path, "rb") as handle:
        actual = hashlib.sha256(handle.read()).hexdigest()
    manifest["tools"][tool] = actual
    expected = digests.get(tool, "REPLACE_WITH_SHA256")
    if expected != actual:
        problems.append(f"{tool}: expected {expected}, found {actual}")

with open(os.path.join(target, "media-tools.manifest.json"), "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2, sort_keys=True)
    handle.write("\n")

if problems:
    print("fetch-media-tools: the fetch did not match the lock file:", file=sys.stderr)
    for problem in problems:
        print(f"  {problem}", file=sys.stderr)
    print(
        "  Replace the REPLACE_WITH_SHA256 entries in media-tools.json with the\n"
        "  digests above once you have checked where they came from.",
        file=sys.stderr,
    )
    sys.exit(1)
print("  digests match the lock file")
PY

echo "fetch-media-tools: ready in $target"
echo "  verify at any time with ./scripts/verify-media-tools.sh $platform"

# The bundler is told about one directory that does not name a platform, so
# tauri.conf.json stays the same on every machine. Only the tools, the manifest,
# and the licence notice are copied into it: the archive that was downloaded is
# a build input, not something to ship.
staged="$vendor/staged"
rm -rf "$staged"
mkdir -p "$staged"
for tool in $tools; do
  cp "$target/$tool" "$staged/$tool"
done
cp "$target/media-tools.manifest.json" "$staged/media-tools.manifest.json"
if [[ -f "$root/LICENSE-MEDIA-TOOLS" ]]; then
  cp "$root/LICENSE-MEDIA-TOOLS" "$staged/LICENSE.txt"
fi
echo "fetch-media-tools: staged for bundling in $staged"