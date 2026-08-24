#!/bin/bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
version="${1:?usage: package_release.sh VERSION [TARGET] [DIST_DIR]}"
target="${2:-$(rustc -vV | sed -n 's/^host: //p')}"
dist_dir="${3:-$repo_root/dist}"
archive_root="archetype-quick-browser-v${version}-${target}"
stage="$dist_dir/$archive_root"

if [[ "$version" != "$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -1)" ]]; then
  echo "release version does not match Cargo.toml" >&2
  exit 1
fi

if [[ -e "$stage" ]]; then
  echo "release staging directory already exists: $stage" >&2
  exit 1
fi
mkdir -p "$stage/bin" "$stage/docs"
for binary in arch-browser archetype-runtime; do
  source="$repo_root/target/release/$binary"
  [[ -x "$source" ]] || { echo "missing release binary: $source" >&2; exit 1; }
  cp "$source" "$stage/bin/$binary"
done
cp "$repo_root/docs/html-css-support.json" "$stage/docs/"
cp "$repo_root/docs/v4-acceptance.md" "$stage/docs/"
cp "$repo_root/LICENSE" "$stage/"
cp "$repo_root/THIRD_PARTY_LICENSES.md" "$stage/"
printf '%s\n' \
  'UNSIGNED DEVELOPMENT BUILD' \
  'This archive is not code-signed or notarized and is not a public production release.' \
  'Keep arch-browser and archetype-runtime together in the same bin directory.' \
  > "$stage/DEVELOPMENT_BUILD.txt"

(
  cd "$stage"
  shasum -a 256 bin/arch-browser bin/archetype-runtime > SHA256SUMS
)
tar -C "$dist_dir" -czf "$dist_dir/$archive_root.tar.gz" "$archive_root"
shasum -a 256 "$dist_dir/$archive_root.tar.gz" > "$dist_dir/$archive_root.tar.gz.sha256"
