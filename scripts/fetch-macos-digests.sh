#!/usr/bin/env bash
# Fetch the two macOS archives and record their digests.
#
# They are published separately, so the pin file lists them separately too, and
# this is what fills those entries in.
set -euo pipefail

work=/tmp/macos-tools
mkdir -p "$work"
cd "$work"

for name in ffmpeg ffprobe; do
  url="https://evermeet.cx/ffmpeg/${name}-9.0.2.zip"
  file="${name}-9.0.2.zip"
  if [[ ! -f "$file" ]]; then
    echo "fetching $url"
    for attempt in 1 2 3 4 5; do
      if curl -fsSL --retry 3 --retry-delay 5 "$url" -o "$file"; then
        break
      fi
      echo "  attempt $attempt failed; waiting"
      sleep 20
    done
  fi
  if [[ ! -s "$file" ]]; then
    echo "could not fetch $url" >&2
    exit 1
  fi
  printf '%s  %s  %s bytes\n' "$(sha256sum "$file" | cut -d' ' -f1)" "$file" "$(stat -c%s "$file")"
done

echo "--- the tools inside them ---"
for file in *.zip; do
  echo "$file:"
  unzip -l "$file" | head -6
done