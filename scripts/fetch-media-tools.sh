#!/usr/bin/env bash
# Fetch the pinned media tools into the desktop crate's vendor/ffmpeg/<platform>/,
# which is where the Tauri bundle is told to look for them.
#
# The pin file names one or more archives per platform. A platform whose tools
# are published as one archive names one; one whose tools are published
# separately names both, and each tool is taken from whichever archive holds it.
#
# usage: ./scripts/fetch-media-tools.sh [platform]

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
  echo "fetch-media-tools: a working python is needed to read the pin file" >&2
  exit 1
fi

host_platform() {
  local os arch
  case "$(uname -s)" in
    Darwin)
      # macOS has one pinned encoder build, and it is the x86_64 one. Apple
      # silicon runs it through Rosetta 2, so the key names the tools rather
      # than the machine: both architectures resolve to the same build, and
      # naming the machine would ask the pin file for a build nobody publishes.
      echo "darwin-x86_64"
      return
      ;;
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

if [[ ! -f "$lock" ]]; then
  echo "fetch-media-tools: no pin file at $lock" >&2
  exit 1
fi

# Everything about the platform is read in one go, so the pin file is the only
# place the archives, the digests, and the tool names are written down.
read_platform() {
  $PYTHON - "$lock" "$platform" "$1" <<'PY'
import json, sys
lock = json.load(open(sys.argv[1], encoding="utf-8"))
entry = lock["platforms"].get(sys.argv[2])
if entry is None:
    known = ", ".join(sorted(lock["platforms"]))
    sys.exit(f"fetch-media-tools: '{sys.argv[2]}' is not in the pin file (known: {known})")

field = sys.argv[3]
if field == "archives":
    # A platform may name one archive or several. The single-archive form is
    # the older one and is still read, so a pin file does not have to be
    # rewritten when a platform gains a second archive.
    archives = entry.get("archives")
    if archives is None:
        archives = [{
            "name": entry["archive"],
            "url": entry["url"],
            "sha256": entry.get("archive_sha256", ""),
        }]
    print(json.dumps(archives))
elif field == "version":
    print(entry.get("version") or entry["archive"])
elif field == "tools":
    print(" ".join(entry["tools"]))
elif field == "digests":
    print(json.dumps(entry["digests"]))
PY
}

archives_json="$(read_platform archives)"
version="$(read_platform version)"
tools="$(read_platform tools)"
digests_json="$(read_platform digests)"

echo "fetch-media-tools: ffmpeg $version for $platform"

target="$vendor/$platform"
mkdir -p "$target" "$vendor/.cache"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Each archive is fetched, checked against its pin, and unpacked into one
# directory, so a tool is taken from whichever archive holds it.
while IFS= read -r archive_json; do
  name="$(echo "$archive_json" | "$PYTHON" -c 'import json,sys; print(json.load(sys.stdin)["name"])')"
  url="$(echo "$archive_json" | "$PYTHON" -c 'import json,sys; print(json.load(sys.stdin)["url"])')"
  digest="$(echo "$archive_json" | "$PYTHON" -c 'import json,sys; print(json.load(sys.stdin).get("sha256",""))')"

  archive_path="$vendor/.cache/$name"
  if [[ ! -f "$archive_path" ]]; then
    echo "  downloading $name"
    if command -v curl >/dev/null 2>&1; then
      curl -fsSL --retry 3 --retry-delay 5 "$url" -o "$archive_path"
    elif command -v wget >/dev/null 2>&1; then
      wget -q "$url" -O "$archive_path"
    else
      echo "fetch-media-tools: neither curl nor wget is available" >&2
      exit 1
    fi
  else
    echo "  $name is already cached"
  fi

  # The archive is checked against the pin before anything is unpacked from it,
  # so a tampered download is refused at the point it arrives rather than after
  # its contents have been trusted once.
  if [[ -n "$digest" && "$digest" != REPLACE_WITH_SHA256 ]]; then
    actual="$($PYTHON - "$archive_path" <<'PY'
import hashlib, sys
with open(sys.argv[1], "rb") as handle:
    print(hashlib.sha256(handle.read()).hexdigest())
PY
)"
    if [[ "$actual" != "$digest" ]]; then
      echo "fetch-media-tools: the downloaded archive is not the pinned one:" >&2
      echo "  expected $digest" >&2
      echo "  found    $actual" >&2
      echo "  Nothing was unpacked from it." >&2
      exit 1
    fi
    echo "  $name matches the pin"
  else
    echo "  warning: the pin file records no digest for $name"
  fi

  case "$name" in
    *.zip) (cd "$work" && unzip -oq "$archive_path") ;;
    *.tar.xz|*.txz) (cd "$work" && tar xf "$archive_path") ;;
    *.tar.gz|*.tgz) (cd "$work" && tar xf "$archive_path") ;;
    *) echo "fetch-media-tools: do not know how to unpack $name" >&2; exit 1 ;;
  esac
done < <("$PYTHON" -c 'import json,sys; [print(json.dumps(a)) for a in json.load(sys.stdin)]' <<<"$archives_json")

# Each tool is taken from the unpacked tree, wherever it ended up.
for tool in $tools; do
  found="$(find "$work" -type f -name "$tool" -print -quit)"
  if [[ -z "$found" ]]; then
    echo "fetch-media-tools: $tool is not in the archives" >&2
    exit 1
  fi
  cp "$found" "$target/$tool"
  chmod +x "$target/$tool" 2>/dev/null || true
  echo "  unpacked $tool"
done

# The manifest is what a release is checked against, so it records what was
# actually fetched rather than what was asked for.
$PYTHON - "$target" "$platform" "$version" "$archives_json" "$digests_json" "$tools" <<'PY'
import hashlib, json, os, sys

target, platform, version, archives_json, digests_json, tools = sys.argv[1:7]
digests = json.loads(digests_json)
archives = json.loads(archives_json)
manifest = {
    "version": version,
    "platform": platform,
    "source": archives[0]["url"] if len(archives) == 1 else [a["url"] for a in archives],
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
    if expected != "REPLACE_WITH_SHA256" and expected != actual:
        problems.append(f"{tool}: expected {expected}, found {actual}")

with open(os.path.join(target, "media-tools.manifest.json"), "w", encoding="utf-8") as handle:
    json.dump(manifest, handle, indent=2, sort_keys=True)
    handle.write("\n")

if problems:
    print("fetch-media-tools: the fetch did not match the pin file:", file=sys.stderr)
    for problem in problems:
        print(f"  {problem}", file=sys.stderr)
    print(
        "  Replace the REPLACE_WITH_SHA256 entries in media-tools.json with the\n"
        "  digests above once you have checked where they came from.",
        file=sys.stderr,
    )
    sys.exit(1)
print("  digests match the pin file")
PY

echo "fetch-media-tools: ready in $target"
echo "  verify at any time with ./scripts/verify-media-tools.sh $platform"

# The bundler is told about one directory that does not name a platform, so
# tauri.conf.json stays the same on every machine. Only the tools, the manifest,
# and the licence notice are copied into it: the archives that were downloaded
# are build inputs, not something to ship.
staged="$vendor/staged"
# The directory is emptied first: the bundle copies it whole, so tools left
# there by a build for another platform would be shipped alongside this one.
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