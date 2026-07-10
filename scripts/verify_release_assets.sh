#!/usr/bin/env bash
set -euo pipefail

assets_dir="${1:?usage: verify_release_assets.sh <assets-dir>}"
if [[ ! -d "$assets_dir" ]]; then
  printf 'release assets directory not found: %s\n' "$assets_dir" >&2
  exit 1
fi

expected_archives="$(printf '%s\n' \
  imagegenexpert-linux-x86_64.zip \
  imagegenexpert-macos-aarch64.zip \
  imagegenexpert-macos-x86_64.zip \
  imagegenexpert-windows-x86_64.zip | LC_ALL=C sort)"
actual_archives="$(find "$assets_dir" -maxdepth 1 -type f -name '*.zip' -printf '%f\n' | LC_ALL=C sort)"

if [[ "$actual_archives" != "$expected_archives" ]]; then
  printf 'unexpected release archive set\nexpected:\n%s\nactual:\n%s\n' \
    "$expected_archives" "$actual_archives" >&2
  exit 1
fi

verify_archive() {
  local archive_name="$1"
  local binary_name="$2"
  local archive="$assets_dir/$archive_name"
  local expected_entries actual_entries

  unzip -tqq "$archive"
  expected_entries="$(printf '%s\n' \
    imagegenexpert/SKILL.md \
    imagegenexpert/agents/openai.yaml \
    "imagegenexpert/bin/$binary_name" | LC_ALL=C sort)"
  actual_entries="$(unzip -Z1 "$archive" | tr '\\' '/' | sed '/\/$/d' | LC_ALL=C sort)"

  if [[ "$actual_entries" != "$expected_entries" ]]; then
    printf 'unexpected archive layout: %s\nexpected:\n%s\nactual:\n%s\n' \
      "$archive_name" "$expected_entries" "$actual_entries" >&2
    exit 1
  fi

  if [[ "$binary_name" != *.exe ]]; then
    local extract_dir
    extract_dir="$(mktemp -d)"
    unzip -qq "$archive" -d "$extract_dir"
    if [[ ! -x "$extract_dir/imagegenexpert/bin/$binary_name" ]]; then
      printf 'packaged Unix binary is not executable: %s\n' "$archive_name" >&2
      rm -rf "$extract_dir"
      exit 1
    fi
    rm -rf "$extract_dir"
  fi
}

verify_archive imagegenexpert-linux-x86_64.zip imagegen
verify_archive imagegenexpert-macos-x86_64.zip imagegen
verify_archive imagegenexpert-macos-aarch64.zip imagegen
verify_archive imagegenexpert-windows-x86_64.zip imagegen.exe

printf 'Verified release assets in %s\n' "$assets_dir"
