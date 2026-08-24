#!/bin/bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
matrix="${1:-$repo_root/docs/sdk-compatibility.json}"
sdk_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo_root/crates/archetype-sdk/Cargo.toml" | head -1)"
runtime_version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -1)"
protocol_major="$(sed -n 's/^pub const PROTOCOL_MAJOR: u16 = \([0-9]*\);/\1/p' "$repo_root/crates/archetype-protocol/src/lib.rs")"
protocol_minor="$(sed -n 's/^pub const PROTOCOL_MINOR: u16 = \([0-9]*\);/\1/p' "$repo_root/crates/archetype-protocol/src/lib.rs")"

jq -e \
  --arg sdk "$sdk_version" \
  --arg runtime "${runtime_version%.*}.x" \
  --argjson major "$protocol_major" \
  --argjson minor "$protocol_minor" '
    .schema_version == 1 and
    .status == "developer_preview" and
    .sdk_version == $sdk and
    .runtime_compatibility == $runtime and
    .protocol_major == $major and
    .protocol_minor == $minor and
    .frame.format == "rgba8" and
    .targets == ["aarch64-apple-darwin"]
  ' "$matrix" >/dev/null
