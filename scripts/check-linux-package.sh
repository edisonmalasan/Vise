#!/usr/bin/env bash
# Check that an installed Linux package is complete and that its parts run.
#
# This runs inside the build image against a package that has been unpacked, so
# it exercises the files the user would actually get rather than the ones the
# build tree happened to hold.
set -uo pipefail

deb_dir="${1:-/src/target/release/bundle/deb}"
root=/tmp/vise-package-check
rm -rf "$root"
mkdir -p "$root"

deb="$(find "$deb_dir" -name '*.deb' -print -quit)"
if [[ -z "$deb" ]]; then
  echo "check: no package found in $deb_dir" >&2
  exit 1
fi
echo "check: unpacking $(basename "$deb")"
dpkg-deb -x "$deb" "$root"

problems=0
note() { printf '  %s\n' "$1"; }
fail() { printf '  FAIL: %s\n' "$1"; problems=$((problems + 1)); }

app="$root/usr/bin/vise-desktop"
resources="$root/usr/lib/Vise Transcoder"

echo "check: the application"
if [[ -x "$app" ]]; then
  note "present at usr/bin/vise-desktop"
else
  fail "the application binary is missing"
fi

echo "check: the worker"
if [[ -x "$resources/vise-worker" ]]; then
  note "present and executable"
  if "$resources/vise-worker" --help 2>&1 | grep -q "started by the Vise desktop application"; then
    note "it runs and knows it is started by the application"
  else
    fail "the worker did not answer as expected"
  fi
else
  fail "the worker is missing"
fi

echo "check: the media tools"
for tool in ffmpeg ffprobe; do
  if [[ -x "$resources/media/$tool" ]]; then
    note "$tool is present"
    # Each tool names itself, so the check asks for that name rather than for a
    # shared one: a copy of ffmpeg standing in for ffprobe would otherwise pass.
    if "$resources/media/$tool" -hide_banner -version 2>&1 | head -1 | grep -q "$tool version"; then
      note "$tool runs and is itself"
    else
      fail "$tool did not run, or was not itself"
    fi
  else
    fail "$tool is missing"
  fi
done

echo "check: the licence and the manifest"
if [[ -f "$resources/media/LICENSE.txt" ]]; then
  note "the media licence is shipped"
else
  fail "the media licence is missing"
fi
if [[ -f "$resources/media/media-tools.manifest.json" ]]; then
  note "the tools manifest is shipped"
else
  fail "the tools manifest is missing"
fi

echo "check: the desktop entry"
if [[ -f "$root/usr/share/applications/Vise Transcoder.desktop" ]]; then
  note "the desktop entry is installed"
else
  fail "the desktop entry is missing"
fi

echo "check: nothing from another platform"
if [[ -e "$resources/vise-worker.exe" ]]; then
  fail "a worker for another platform was shipped"
else
  note "no foreign worker was shipped"
fi

if [[ $problems -eq 0 ]]; then
  echo "check: the package is complete"
  exit 0
fi
echo "check: $problems problem(s)" >&2
exit 1