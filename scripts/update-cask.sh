#!/usr/bin/env bash
#
# Update homebrew/Casks/headroom.rb to a released version.
#
# Homebrew needs the exact version + sha256 of the published DMG. This script
# downloads the released aarch64 DMG, computes its sha256, and rewrites the two
# pinned lines in the cask. Run it after a release is published, then copy the
# cask into the homebrew-headroom tap (see homebrew/README.md).
#
# Usage:
#   scripts/update-cask.sh 1.5.2
#   scripts/update-cask.sh            # defaults to version in src-tauri/tauri.conf.json
#
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cask="$repo_root/homebrew/Casks/headroom.rb"

version="${1:-}"
if [[ -z "$version" ]]; then
  version="$(grep -m1 '"version"' "$repo_root/src-tauri/tauri.conf.json" | sed -E 's/.*"version": *"([^"]+)".*/\1/')"
fi
[[ -n "$version" ]] || { echo "Could not determine version" >&2; exit 1; }

url="https://github.com/allandecastro/headroom/releases/download/v${version}/Headroom_${version}_aarch64.dmg"
tmp="$(mktemp -t headroom-dmg)"
trap 'rm -f "$tmp"' EXIT

echo "Downloading $url"
curl -fSL -o "$tmp" "$url"
sha="$(shasum -a 256 "$tmp" | awk '{print $1}')"
echo "version=$version"
echo "sha256=$sha"

# Rewrite the two pinned lines in the cask.
sed -i '' -E "s/^  version \".*\"/  version \"${version}\"/" "$cask"
sed -i '' -E "s/^  sha256 \".*\"/  sha256 \"${sha}\"/" "$cask"

echo "Updated $cask"
