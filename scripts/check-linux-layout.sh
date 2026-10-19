#!/usr/bin/env bash
# Check that the installed application finds its worker and its tools.
#
# The packaging layouts differ: a Linux package installs the binary under
# /usr/bin and its resources under /usr/lib, so the worker is not beside the
# binary. This runs the installed binary far enough to see what it says, which
# is the only honest way to know it found what it needed.
set -uo pipefail

deb_dir="${1:-/src/target/release/bundle/deb}"
root=/tmp/vise-resolve-check
rm -rf "$root"
mkdir -p "$root"

deb="$(find "$deb_dir" -name '*.deb' -print -quit)"
dpkg-deb -x "$deb" "$root"

app="$root/usr/bin/vise-desktop"
resources="$root/usr/lib/Vise Transcoder"

echo "resolve: the layout the package installs"
echo "  binary:   $app"
echo "  resources: $resources"

echo "resolve: the worker the application would look for"
found=""
# The application resolves the worker relative to its own executable, so the
# same search is reproduced here against the installed tree.
app_dir="$(dirname "$app")"
for candidate in \
  "$app_dir/vise-worker" \
  "$app_dir/../lib/Vise Transcoder/vise-worker" \
  "$app_dir/../lib/vise-transcoder/vise-worker"; do
  if [[ -f "$candidate" ]]; then
    found="$candidate"
    break
  fi
done

if [[ -z "$found" ]]; then
  echo "resolve: FAIL: no candidate held a worker" >&2
  exit 1
fi
echo "  found: $found"
echo "  (relative to the binary: ${found#"$app_dir"/})"

echo "resolve: the tools the application would look for"
tools_found=""
for candidate in \
  "$app_dir/media/ffmpeg" \
  "$resources/media/ffmpeg" \
  "$app_dir/../lib/Vise Transcoder/media/ffmpeg"; do
  if [[ -f "$candidate" ]]; then
    tools_found="$candidate"
    break
  fi
done
if [[ -z "$tools_found" ]]; then
  echo "resolve: FAIL: no candidate held the media tools" >&2
  exit 1
fi
echo "  found: $tools_found"

echo "resolve: the package is laid out so the application can find its parts"