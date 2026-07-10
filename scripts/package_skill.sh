#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
out_dir="${1:-"$repo_dir/dist/imagegenexpert"}"

rm -rf "$out_dir"
mkdir -p "$out_dir/bin" "$out_dir/agents"

cp "$repo_dir/SKILL.md" "$out_dir/SKILL.md"
cp "$repo_dir/agents/openai.yaml" "$out_dir/agents/openai.yaml"
output_binary="${IMAGEGEN_OUTPUT_BINARY:-imagegen}"

if [[ -n "${IMAGEGEN_BIN_OVERRIDE:-}" ]]; then
  bin_path="$IMAGEGEN_BIN_OVERRIDE"
else
  cargo build --release
  bin_path="$repo_dir/target/release/imagegen"
fi

cp "$bin_path" "$out_dir/bin/$output_binary"
chmod +x "$out_dir/bin/$output_binary"

printf 'Packaged imagegenexpert runtime at %s\n' "$out_dir"
