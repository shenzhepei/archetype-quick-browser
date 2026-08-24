#!/bin/bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
probe="${1:-$repo_root/target/debug/archetype-sandbox-probe}"
profile="$repo_root/config/macos/runtime.sb"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "runtime sandbox probe requires macOS" >&2
  exit 2
fi
if [[ ! -x "$probe" ]]; then
  echo "sandbox probe is not executable: $probe" >&2
  exit 2
fi

secret="$(mktemp -t archetype-runtime-secret)"
trap 'rm -f "$secret"' EXIT
printf 'sandbox-probe-secret\n' > "$secret"

"$probe" "$secret" "1.1.1.1:443"
/usr/bin/sandbox-exec \
  -D "EXECUTABLE=$probe" \
  -f "$profile" \
  "$probe" "$secret" "1.1.1.1:443" --expect-blocked
