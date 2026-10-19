#!/usr/bin/env bash
# Print the release notes for one version.
#
# The notes a release carries live in release-notes/<version>.md, and the
# release workflow publishes exactly that file. This script reads the same
# place, so what is read here is what reaches a user.
#
# usage: ./scripts/release-notes.sh <version>

set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
directory="$root/release-notes"

version="${1:-}"
if [[ -z "$version" ]]; then
  echo "release-notes: a version is needed" >&2
  exit 1
fi
# A tag carries a "v" and a version does not, so both spellings find the
# same notes.
version="${version#v}"

notes="$directory/$version.md"
if [[ ! -f "$notes" ]]; then
  echo "release-notes: no notes at $notes" >&2
  echo "  write them before publishing, so the release is not empty" >&2
  exit 1
fi

cat "$notes"
